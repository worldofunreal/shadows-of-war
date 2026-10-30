import { setAvatarPortrait } from "./avatar-presentation.ts";
import { renderLocaleSettingRow } from "./main_menu.settings.ts";
import { syncDropdownScope } from "./sow-dropdown.ts";
import type {
    HudAvatarIdentity,
    HudBuildingPlacement,
    HudBuildingProgressItem,
    HudBuildingCelebrationItem,
    HudCombatOperation,
    HudDevTools,
    HudInboxRequest,
    HudLeaderboardPlayer,
    HudLeaderboardTeam,
    HudMapFeedback,
    HudMapMenu,
    HudMapMenuBuilding,
    HudMapMenuItem,
    HudNotification,
    HudStateEnvelope,
    HudDevParameter,
    MenuHudPayload,
    MenuLeader,
    MenuSettings,
} from "./main_menu.state.ts";

type HudPanel = "leaderboard" | "inbox" | "settings" | null;
type HudCommandValue = string | number | boolean | null | number[] | string[];
type HudCommandPayload = Record<string, HudCommandValue>;
type HudScreenSize = { width: number; height: number };
type HudEndgameStats = NonNullable<MenuHudPayload["player_endgame_stats"]>;
type HudEndgameRefs = Record<
    "kills" | "assists" | "maxTerritory" | "goldReceived" | "troopsReceived",
    HTMLElement | null
>;
type HudNotificationAvatar = { wrapper: HTMLElement; image: HTMLImageElement; emblem: HTMLElement };
type HudNotificationCard = {
    card: HTMLElement;
    seal: HTMLElement;
    avatars: HudNotificationAvatar[];
    copy: HTMLElement;
    renderKey: string;
};
type ActiveHudNotification = {
    entry: HudNotification | HudMapFeedback;
    priority: number;
    group: string;
    sumValues: boolean;
    expiresAt: number;
    createdAt: number;
    totals: Record<string, number> | null;
};
type ActiveMapFeedback = { entry: HudMapFeedback; priority: number; expiresAt: number };
type NotificationRenderItem = { entry: HudNotification | HudMapFeedback; priority: number };
type PlayerIdentityParts = {
    element: HTMLElement;
    portrait: HTMLImageElement;
    emblem: HTMLElement;
    copy: HTMLElement;
    name: HTMLElement;
};
type InboxRequestParts = PlayerIdentityParts & {
    type: HTMLElement;
    resources: HTMLElement;
    amounts: Record<"gold" | "troops", { element: HTMLElement; value: HTMLElement }>;
    accept: HTMLButtonElement;
    reject: HTMLButtonElement;
};
type LeaderboardRow = {
    card: HTMLElement;
    rank: HTMLElement;
    avatarWrap: HTMLElement;
    avatarImg: HTMLImageElement;
    avatarBadge: HTMLElement;
    name: HTMLElement;
    territory: HTMLElement;
    troops: HTMLElement;
    gpsBtn: HTMLButtonElement;
    transferBtn: HTMLButtonElement;
    key: string;
};
type CombatOperationRow = {
    element: HTMLElement;
    portrait: HTMLImageElement;
    emblem: HTMLElement;
    glyph: HTMLElement;
    name: HTMLElement;
    details: HTMLElement;
    detailsLabel: HTMLElement;
    detailsSeparator: HTMLElement;
    detailsEta: HTMLElement;
    troops: HTMLElement;
    actions: HTMLElement;
    actionKey: string;
};
type RingEmote = NonNullable<MenuHudPayload["emotes"]>[number];
type RingActionSlot = {
    type: "action";
    item: HudMapMenuItem;
    presentation: { state: string; key: string; status: string; disabled: boolean } | null | undefined;
};
type HudActionReasonItem = Partial<HudMapMenuItem>;
type RingSlot =
    | RingActionSlot
    | { type: "group"; group: "build" | "emote" }
    | { type: "emote"; definition: RingEmote }
    | { type: "pin"; enabled: boolean }
    | { type: "clear" };
type RingPress = { id: number | null; fromOpen: boolean };
type RingHold = { index: number; started: number };
type CompleteHudDevTools = HudDevTools & {
    schema: NonNullable<HudDevTools["schema"]>;
    groups: NonNullable<HudDevTools["groups"]>;
};
type HudRingState = Partial<
    Pick<MenuHudPayload, "emotes" | "emote_active" | "emote_pinned" | "player_leader" | "gold">
> & {
    map_menu?: HudMapMenu | null;
};
type HudRefs = {
    fps: HTMLElement;
    ping: HTMLElement;
    diagnostics: HTMLElement;
    nameplate: HTMLElement;
    plateAvatar: HTMLImageElement;
    plateName: HTMLElement;
    plateFill: HTMLElement;
    plateTroops: HTMLElement;
    plateGold: HTMLElement;
    plateGoldWrap: HTMLElement;
    plateGoldRate: HTMLElement;
    plateTroopRate: HTMLElement;
    inboxCount: HTMLElement;
    hoverCard: HTMLElement;
    hoverName: HTMLElement;
    hoverPct: HTMLElement;
    hoverTroops: HTMLElement;
    hoverGold: HTMLElement;
    hoverBlds: HTMLElement;
    leftRail: HTMLElement;
    slider: HTMLInputElement;
    armyAllocation: HTMLElement;
    armyRatio: HTMLElement;
    rightRail: HTMLElement;
    devSidebar: HTMLElement;
    devBtn: HTMLElement;
    settings: HTMLElement;
    deployBtn: HTMLElement;
    deployTimer: HTMLElement;
    buildingsStrip: HTMLElement;
    worldProgressOverlay: HTMLElement;
    worldCelebrationsOverlay: HTMLElement;
    buildingButtons: HTMLButtonElement[];
    fleetStrip: HTMLElement;
    fleetTrade: HTMLElement;
    fleetWarship: HTMLButtonElement;
    fleetNuke: HTMLButtonElement;
    panelButtons: HTMLButtonElement[];
    leaderboard: HTMLElement;
    rows: HTMLElement;
    lbSearch: HTMLInputElement;
    lbEmpty: HTMLElement;
    inbox: HTMLElement;
    inboxRows: HTMLElement;
    inboxClose: HTMLElement;
    inboxButton: HTMLElement;
    inboxTotal: HTMLElement;
    inboxEmpty: HTMLElement;
    inboxRequests: Record<string, InboxRequestParts>;
    inboxRequestKeys: string[];
    inboxRenderKey: string;
    transfer: HTMLElement;
    transferTarget: HTMLElement;
    transferClose: HTMLElement;
    transferReturnFocus: HTMLElement | null;
    transferGold: HTMLInputElement;
    transferTroops: HTMLInputElement;
    transferIdentity: PlayerIdentityParts;
    betrayal: HTMLElement;
    betrayalTarget: HTMLElement;
    betrayalCancel: HTMLElement;
    betrayalReturnFocus: HTMLElement | null;
    betrayalIdentity: PlayerIdentityParts;
    surrender: HTMLElement;
    surrenderTitle: HTMLElement;
    surrenderDesc: HTMLElement;
    surrenderStats: HTMLElement;
    surrenderStatValues: HudEndgameRefs;
    surrenderSaveNote: HTMLElement;
    surrenderPortrait: HTMLImageElement;
    surrenderCancel: HTMLElement;
    surrenderActionLabel: HTMLElement;
    endgame: HTMLElement;
    endgameCard: HTMLElement;
    endgameBanner: HTMLElement;
    endgamePortrait: HTMLImageElement;
    endgameStatValues: HudEndgameRefs;
    endgameStore: HTMLElement;
    endgameStoreName: HTMLElement;
    endgameStoreCopy: HTMLElement;
    endgameStoreAction: HTMLElement;
    endgameObserve: HTMLButtonElement;
    mapMenu: HTMLElement;
    placementOverlay: HTMLElement;
    placementPanel: HTMLElement;
    placementArrows: Record<"up" | "right" | "down" | "left", HTMLButtonElement>;
    operations: HTMLElement;
    operationRows: HTMLElement;
    combatOperationRows: Record<string, CombatOperationRow>;
    combatOperationsOrderKey: string;
    notifications: HTMLDivElement;
    topbar: HTMLElement;
    dock: HTMLElement;
    dockStack: HTMLElement;
    mapFeedback: HTMLDivElement;
};

function requiredHudElement(id: string): HTMLElement {
    const element = document.getElementById(id);
    if (!(element instanceof HTMLElement)) throw new Error("Missing HUD element: " + id);
    return element;
}

function requiredHudInput(id: string): HTMLInputElement {
    const element = requiredHudElement(id);
    if (!(element instanceof HTMLInputElement)) throw new Error("HUD element is not an input: " + id);
    return element;
}

function requiredHudImage(id: string): HTMLImageElement {
    const element = requiredHudElement(id);
    if (!(element instanceof HTMLImageElement)) throw new Error("HUD element is not an image: " + id);
    return element;
}

function requiredHudButton(id: string): HTMLButtonElement {
    const element = requiredHudElement(id);
    if (!(element instanceof HTMLButtonElement)) throw new Error("HUD element is not a button: " + id);
    return element;
}

function requiredHudButtonQuery(root: ParentNode, selector: string): HTMLButtonElement {
    const element = root.querySelector(selector);
    if (!(element instanceof HTMLButtonElement)) throw new Error("Missing HUD button: " + selector);
    return element;
}

function requiredHudQuery(root: ParentNode, selector: string): HTMLElement {
    const element = root.querySelector(selector);
    if (!(element instanceof HTMLElement)) throw new Error("Missing HUD element: " + selector);
    return element;
}

(function () {
    "use strict";

    // ─────────────────────────────────────────────────────────
    // HUD CONTROLLER (DOM Overlay for In-Game RTS Matches)
    // ─────────────────────────────────────────────────────────
    const hudRoot = document.getElementById("sow-hud");
    let hudDevLayer = document.getElementById("sow-hud-dev-layer");
    let hudState: HudStateEnvelope | null = null;
    let lastHudRaw = "";
    let hudScreenSize: HudScreenSize | null = null;
    let activeHudPanel: HudPanel = null;
    let betrayalOpen = false;
    let emoteTextKey = "";
    let emoteTextTarget: Window["SOW_menu_command"] | null = null;
    let hudSafeArea = [0, 0, 0, 0];
    let surrenderModalOpen = false;
    let surrenderMessage: string | null = null;
    let devSidebarOpen = false;
    let devToolsSchemaKey = "";
    let devTab = "";
    let devControlIndex: Record<string, HTMLElement> = Object.create(null);
    let devStepTimer = 0;
    let devResetTimer = 0;
    let hudDropdownOpenKey: string | null = null;

    let hudInitialized = false;
    let hudRefs: ReturnType<typeof createHudRefs> | null = null;
    let leaderboardRows: Record<string, LeaderboardRow> = Object.create(null);
    let leaderboardRenderKey = "";
    let leaderboardFilterQuery = "";
    let pendingCounterAttacks: Record<string, number> = Object.create(null);
    let lastLeaderboardPlayers: HudLeaderboardPlayer[] = [];
    let lastLeaderboardTeams: HudLeaderboardTeam[] = [];
    let notificationCursor = 0;
    let notificationTimer: number | null = null;
    let activeNotifications: ActiveHudNotification[] = [];
    let notificationCards: HudNotificationCard[] = [];
    let notificationLayoutFrame = 0;
    let notificationResizeObserver: ResizeObserver | null = null;
    let notificationMutationObserver: MutationObserver | null = null;
    let notificationLayoutStarted = false;
    let notificationLayoutStory: HTMLElement | null = null;
    let notificationLayoutObjective: HTMLElement | null = null;
    let notificationLayoutNameplate: HTMLElement | null = null;
    let notificationLayoutNotifications: HTMLElement | null = null;
    let notificationLayoutState = "";
    let mapFeedbackCursor = 0;
    let mapFeedbackTimer: number | null = null;
    let activeMapFeedback: ActiveMapFeedback | null = null;
    let mapFeedbackCard: HudNotificationCard | null = null;
    const avatarIdentityKeys = new WeakMap<HTMLImageElement, string>();
    const MAP_FEEDBACK_DURATION_MS = 2000;
    const MAP_FEEDBACK_FADE_OUT_MS = 180;
    let mapMenuView: "root" | "build" | "emote" = "root";
    const ring = {
        session: "",
        hot: -1,
        slots: [] as (RingSlot | null)[],
        press: null as RingPress | null,
        hold: null as RingHold | null,
        holdConfirmed: false,
        pointerDown: false,
        pointerId: null as number | null,
    };
    const allocationHoverNone = window.matchMedia ? window.matchMedia("(hover: none)") : null;
    let lastAllocationKey = "";
    const EMPTY_CAMPAIGN_UNLOCKS: { buildings: Record<string, number>; actions: string[] } = {
        buildings: {},
        actions: [],
    };
    const DEFAULT_HUD_SETTINGS: MenuSettings = {
        mute_all: false,
        music_volume: 0.8,
        reduced_motion: false,
        free_zoom_out: false,
        show_fps_ping: false,
        show_dev_tools: false,
        show_dev_tools_available: false,
    };

    const surrenderMessageKeys = [
        "endgame.match_message_1",
        "endgame.match_message_2",
        "endgame.match_message_3",
        "endgame.match_message_4",
        "endgame.match_message_5",
    ];

    function send(type: string, extra?: HudCommandPayload): boolean {
        if (typeof window.SOW_menu_command !== "function") return false;
        const command = Object.assign({ type: type }, extra || {});
        window.SOW_menu_command(JSON.stringify(command));
        return true;
    }

    function updateHudSafeArea() {
        if (!hudRoot || typeof window.getComputedStyle !== "function") return;
        const probe = document.createElement("span");
        probe.style.cssText =
            "position:absolute;visibility:hidden;pointer-events:none;padding:env(safe-area-inset-top,0px) env(safe-area-inset-right,0px) env(safe-area-inset-bottom,0px) env(safe-area-inset-left,0px)";
        hudRoot.appendChild(probe);
        const style = window.getComputedStyle(probe);
        hudSafeArea = [style.paddingTop, style.paddingRight, style.paddingBottom, style.paddingLeft].map(
            function (value) {
                return Math.max(0, parseFloat(value) || 0);
            },
        );
        probe.remove();
        syncEmoteTexts(hudState && hudState.hud);
    }

    function syncEmoteTexts(hud: MenuHudPayload | null | undefined) {
        if (!hud || !Array.isArray(hud.emotes) || hud.emotes.length !== 4) return;
        const tutorialActive = hud.tutorial && hud.tutorial.active;
        const texts = hud.emotes.map(function (definition) {
            return tutorialActive && typeof definition.fallback === "string"
                ? definition.fallback
                : SOW_t(definition.bubble_key);
        });
        const key =
            String(tutorialActive ? "en" : window.SOW_LOCALE || "en") +
            ":" +
            JSON.stringify(texts) +
            ":" +
            hudSafeArea.join(",");
        if (key === emoteTextKey && emoteTextTarget === window.SOW_menu_command) return;
        if (send("set_emote_texts", { texts: texts, safe_area: hudSafeArea })) {
            emoteTextKey = key;
            emoteTextTarget = window.SOW_menu_command;
        }
    }

    function setHudPanel(panel: HudPanel) {
        const previous = activeHudPanel;
        const next = previous === panel ? null : panel;
        if (next !== "settings" && hudDropdownOpenKey) {
            hudDropdownOpenKey = null;
            if (hudRefs && hudRoot) syncDropdownScope(hudRoot, null);
        }
        if (previous === "leaderboard" && next !== previous) send("toggle_leaderboard");
        if (previous === "inbox" && next !== previous) send("toggle_inbox");
        activeHudPanel = next;
        if (next && devSidebarOpen) {
            devSidebarOpen = false;
            send("toggle_dev_sidebar");
        }
        if (next === "leaderboard") send("toggle_leaderboard");
        if (next === "inbox") send("toggle_inbox");
    }

    function setHudDropdownOpen(key: string | null, focusTarget?: HTMLElement | null) {
        hudDropdownOpenKey = key;
        if (hudRoot) syncDropdownScope(hudRoot, key);
        if (focusTarget) focusTarget.focus({ preventScroll: true });
    }

    function selectHudDropdownOption(key: string, value: string) {
        if (!hudRoot) return;
        const dropdowns = hudRoot.querySelectorAll<HTMLElement>("[data-control-dropdown]");
        let dropdown: HTMLElement | null = null;
        for (let i = 0; i < dropdowns.length; i++) {
            if (dropdowns[i].dataset.dropdownKey === key) {
                dropdown = dropdowns[i];
                break;
            }
        }
        if (!dropdown) return;
        const input = dropdown.querySelector<HTMLInputElement>("[data-dropdown-input]");
        if (!input) return;
        input.value = value;
        const trigger = dropdown.querySelector<HTMLElement>("[data-role='dropdown-trigger']");
        setHudDropdownOpen(null, trigger);
        input.dispatchEvent(new Event("change", { bubbles: true }));
    }

    function handleHudDropdownKeydown(event: KeyboardEvent): boolean {
        const target = event.target;
        if (!(target instanceof HTMLElement) || !hudRoot || !hudRoot.contains(target)) return false;
        const trigger = target.closest<HTMLButtonElement>("[data-role='dropdown-trigger']");
        const option = target.closest<HTMLButtonElement>("[data-command='select_dropdown']");
        if (trigger) {
            if (["ArrowDown", "ArrowUp", "Enter", " "].indexOf(event.key) < 0) return false;
            const key = trigger.dataset.dropdownKey;
            if (!key) return false;
            event.preventDefault();
            setHudDropdownOpen(key);
            const dropdown = trigger.closest<HTMLElement>("[data-control-dropdown]");
            const options = dropdown
                ? Array.from(dropdown.querySelectorAll<HTMLButtonElement>("[data-command='select_dropdown']"))
                : [];
            const selected = options.find((entry) => entry.getAttribute("aria-selected") === "true");
            const next = event.key === "ArrowUp" ? options[options.length - 1] : selected || options[0];
            if (next) next.focus({ preventScroll: true });
            return true;
        }
        if (option) {
            const dropdown = option.closest<HTMLElement>("[data-control-dropdown]");
            const options = dropdown
                ? Array.from(dropdown.querySelectorAll<HTMLButtonElement>("[data-command='select_dropdown']"))
                : [];
            const index = options.indexOf(option);
            if (event.key === "Escape") {
                event.preventDefault();
                const trigger = dropdown?.querySelector<HTMLElement>("[data-role='dropdown-trigger']");
                setHudDropdownOpen(null, trigger);
                return true;
            }
            if (event.key === "Tab") {
                setHudDropdownOpen(null);
                return false;
            }
            let nextIndex = index;
            if (event.key === "ArrowDown") nextIndex = Math.min(options.length - 1, index + 1);
            else if (event.key === "ArrowUp") nextIndex = Math.max(0, index - 1);
            else if (event.key === "Home") nextIndex = 0;
            else if (event.key === "End") nextIndex = options.length - 1;
            if (nextIndex !== index) {
                event.preventDefault();
                options[nextIndex]?.focus({ preventScroll: true });
                return true;
            }
            if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                option.click();
                return true;
            }
        } else if (event.key === "Escape" && hudDropdownOpenKey) {
            event.preventDefault();
            const trigger = hudRoot.querySelector<HTMLElement>(
                "[data-control-dropdown][data-dropdown-key='" +
                    hudDropdownOpenKey +
                    "'] [data-role='dropdown-trigger']",
            );
            setHudDropdownOpen(null, trigger);
            return true;
        }
        return false;
    }

    function asset(path: string): string {
        const base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
        const url = base + "/" + path.split("/").map(encodeURIComponent).join("/");
        return window.SOW_assetUrl ? window.SOW_assetUrl(url) : url;
    }

    function currencyAsset(kind: string): string {
        return asset("gameplay/currency/" + kind + ".webp");
    }

    // One icon per concept: painted art for resources, the shared pixel
    // atlases for relationships and buildings, white glyphs for actions.
    const HUD_ICONS: Record<string, string> = {
        troops: "gameplay/icons/battle_icon_cutout.webp",
        gold: "gameplay/currency/gold.webp",
        surrender: "gameplay/icons/surrender_flag.webp",
    };

    // Cell order of the badge strip packed from assets/art/icons (assets/generated/badges-atlas.json).
    const STATUS_CELLS: string[] = [
        "request",
        "allied",
        "traitor",
        "rank-1",
        "rank-2",
        "rank-3",
        "you",
        "disconnected",
        "upgrade-sparkle",
        "wilderness",
        "troops",
        "death",
        "radiation",
        "construction",
        "hammer",
        "arrow-up",
        "arrow-right",
        "arrow-down",
        "arrow-left",
        "cancel",
        "confirm",
        "gold",
        "boat",
        "clock",
        "shield",
        "range",
        "speed",
    ];

    const BUILDING_GLYPHS: Record<string, string> = {
        city: "city",
        factory: "factory",
        port: "anchor",
        tower: "defense",
        farm: "farm",
    };

    const HUD_GLYPHS: Record<string, string> = {
        plus:
            '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
            '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
            '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
            '<path d="M7 10l6 0"/>' +
            '<path d="M10 7l0 6"/>' +
            '<path d="M21 21l-6 -6"/>' +
            '</svg>',
        minus:
            '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
            '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
            '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
            '<path d="M7 10l6 0"/>' +
            '<path d="M21 21l-6 -6"/>' +
            '</svg>',
        home:
            '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">' +
            '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
            '<path d="M12.707 2.293l9 9c.63 .63 .184 1.707 -.707 1.707h-1v6a3 3 0 0 1 -3 3h-10a3 3 0 0 1 -3 -3v-6h-1c-.89 0 -1.337 -1.077 -.707 -1.707l9 -9a1 1 0 0 1 1.414 0m.793 8.707h-3a1.5 1.5 0 0 0 -1.5 1.5v3a1.5 1.5 0 0 0 1.5 1.5h3a1.5 1.5 0 0 0 1.5 -1.5v-3a1.5 1.5 0 0 0 -1.5 -1.5"/>' +
            '</svg>',
        zoom_in:
            '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
            '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
            '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
            '<path d="M7 10l6 0"/>' +
            '<path d="M10 7l0 6"/>' +
            '<path d="M21 21l-6 -6"/>' +
            '</svg>',
        zoom_out:
            '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
            '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
            '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
            '<path d="M7 10l6 0"/>' +
            '<path d="M21 21l-6 -6"/>' +
            '</svg>',
        close: '<path d="M18 6 6 18M6 6l12 12"/>',
        inbox:
            '<svg class="sow-hud__action-icon" viewBox="0 0 36 39" fill="currentColor" aria-hidden="true">' +
            '<path d="M26.9474 14.6637C27.3732 15.5695 27.9073 16.266 28.5497 16.746C29.1885 17.226 29.7911 17.5183 30.3505 17.6266C30.9099 17.7348 31.4981 18.0416 32.1116 18.5468C32.7251 19.052 33.1906 19.7882 33.5118 20.7482C34.1 22.401 33.1112 24.5518 30.549 27.1935C27.9867 29.8351 24.6269 31.9282 20.4624 33.4764C16.0849 35.0246 12.126 35.6236 8.57499 35.2772C5.02753 34.9271 2.95969 33.9275 2.37146 32.2747C1.83735 30.8348 1.99614 29.3515 2.85143 27.8322C3.70671 26.3129 3.94489 24.8297 3.57319 23.3898C2.07914 18.2653 1.45121 14.2631 1.693 11.3833C1.93479 8.5035 3.43966 5.94125 6.21483 3.69658C6.90772 3.10835 7.29386 2.42989 7.37686 1.654C7.45626 0.881715 7.8424 0.358439 8.5389 0.091388C9.17766 -0.121531 9.79115 0.037256 10.3794 0.571359C10.9676 1.10546 11.7146 1.34364 12.6205 1.29312C16.1426 1.23899 18.7843 2.11953 20.5454 3.93476C22.3029 5.7536 24.4393 9.32631 26.9474 14.6637ZM19.5024 30.8348C21.8518 29.9795 23.9701 28.8463 25.8648 27.4317C27.7594 26.017 29.0946 24.8045 29.8669 23.7904C30.6392 22.7763 30.9748 22.1087 30.8666 21.7875C30.6536 21.1993 30.0004 20.759 28.907 20.4667C27.8135 20.1744 26.1571 20.1599 23.9449 20.427C21.7327 20.694 19.2354 21.3328 16.4602 22.3469C13.7392 23.3609 11.4295 24.523 9.53493 25.8294C7.64031 27.1357 6.38805 28.2581 5.77456 29.1928C5.16106 30.1274 4.93371 30.8348 5.09249 31.3147C5.20076 31.6359 5.86478 31.9282 7.09538 32.1953C8.32237 32.4623 10.1087 32.5165 12.4581 32.3541C14.8074 32.1917 17.1531 31.6864 19.5024 30.8348ZM17.0196 23.8698C17.2325 23.8156 17.514 23.7218 17.8604 23.5883C18.2069 23.4548 18.4342 23.3609 18.5425 23.3068L18.6219 23.3862C18.9972 24.4544 18.7698 25.5623 17.9398 26.7063C17.1134 27.8539 15.9261 28.6947 14.3779 29.2288C11.8157 30.1888 9.78754 30.0011 8.2935 28.6695C10.3758 26.8579 13.2845 25.2556 17.0196 23.8698Z"/>' +
            '</svg>',
        emote: '<path d="M20 11.5a7.5 7.5 0 0 1-8 7.5 8.7 8.7 0 0 1-3.5-.7L4 20l1.4-3.6A7 7 0 0 1 4 12c0-4.1 3.6-7.5 8-7.5s8 3.1 8 7Z"/><path d="M9 11h.01M15 11h.01M9 14.5s1 1.5 3 1.5 3-1.5 3-1.5"/>',
        rankings:
            '<svg class="sow-hud__action-icon" viewBox="0 0 48 41" fill="currentColor" aria-hidden="true">' +
            '<path d="M3.43115 14.2054L5.09646 35.0217H15.0883V14.2054H3.43115Z"/>' +
            '<path d="M15.6436 1.71558C16.4762 12.2625 18.1415 33.6895 18.1415 35.0217H31.464L33.1293 1.71558H15.6436Z"/>' +
            '<path d="M44.2317 20.034H33.4072V34.1891H42.5664L44.2317 20.034Z"/>' +
            '</svg>',
        leaderboard:
            '<svg class="sow-hud__action-icon" viewBox="0 0 48 41" fill="currentColor" aria-hidden="true">' +
            '<path d="M3.43115 14.2054L5.09646 35.0217H15.0883V14.2054H3.43115Z"/>' +
            '<path d="M15.6436 1.71558C16.4762 12.2625 18.1415 33.6895 18.1415 35.0217H31.464L33.1293 1.71558H15.6436Z"/>' +
            '<path d="M44.2317 20.034H33.4072V34.1891H42.5664L44.2317 20.034Z"/>' +
            '</svg>',
        cup:
            '<svg class="sow-hud__action-icon" viewBox="0 0 36 34" fill="currentColor" aria-hidden="true">' +
            '<path fill-rule="evenodd" clip-rule="evenodd" d="M28.8428 0H7.44094V2.48287L1.02324 2.48287C0.45812 2.48287 0 2.94099 0 3.50611V14.129C0 18.63 5.02309 22.3338 9.95271 22.7191C11.571 24.453 14.4175 25.7125 16.951 26.2496V29.2936C14.5283 30.0377 10.2008 31.786 9.95271 33.6909H26.5792C26.3287 31.786 21.5603 30.0353 19.1328 29.2936V26.2496C21.6759 25.7125 24.6501 24.453 26.2708 22.7191C31.198 22.3338 36 18.63 36 14.129V3.50611C36 2.94099 35.5419 2.48287 34.9768 2.48287H28.8428V0ZM2.28495 4.68973H7.26755V14.7214C7.26755 16.701 7.77809 18.5842 8.67636 20.2796C4.94844 19.0924 2.28495 16.0339 2.28495 12.4553V4.68973ZM21.8016 16.925L18.0825 14.8756L14.3593 16.925L15.0702 12.5902L12.0599 9.52209L16.2189 8.88873L18.0825 4.94889L19.942 8.88873L24.101 9.52209L21.0927 12.5902L21.8016 16.925ZM27.328 20.5999C28.2311 18.8274 28.7489 16.8672 28.7489 14.7937V4.75441H33.7435V12.4264C33.7435 16.1664 31.068 19.3597 27.328 20.5999Z"/>' +
            '</svg>',
        search: '<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/>',
        settings:
            '<svg class="sow-hud__action-icon" viewBox="70 135 860 775" fill="currentColor" aria-hidden="true">' +
            '<path d="M341,205c22.61-2.97,45.75-.43,68,4c5.93-17.06,20.41-35.48,30-51c30.77,6.9,63.06,23.94,88.06,42.94c9.99,7.59,18.86,16.75,28.94,24.06c-2.72,13.76-24.17,40.64-32,54c6.35,9.77,13.98,18.47,19.98,28.53C557.54,330.24,564.77,353.8,572,379c8.03,1.11,55.3,6.03,58.86,9.19c3.64,3.22,3.43,16.65,3.46,21.41c.21,39.07-5.77,74.39-18.32,111.4c-22.08-1.42-44.14-5.63-66-9c-8.44,21.72-26.54,52.27-48,63c1.83,11.12,25.68,53.57,23,60c-.82,1.97-11.84,8.03-14.18,9.49C478.2,664.88,439.36,684.31,400,684c-4.9-11.59-22-44.37-22-55c-23.77,.37-45.47-1.93-69-4c-9.82,17.51-19.87,35.3-31,52c-43.13-8.12-85.06-41.51-115-72c4-10.54,31.11-47.38,31-53c-.04-2.19-17.99-25.01-20.55-29.44C160.3,499.79,151.48,475.94,147,450c-8.18-1.7-58.86-8.15-61.62-10.63c-1.35-1.21-1.87-11.53-2.05-13.79C80.34,387.45,88.66,341.76,109,309c8.84,.39,57.25,10.02,61,7c3.88-3.13,8.56-14.9,11.67-19.63C191.1,282.04,202.55,268.7,216,258c-2.51-8.07-23.64-52.49-22-57c.65-1.79,8.44-6.25,10.43-7.63C231.08,174.97,284.22,147.01,317,148c4.84,11.37,24,47.28,24,57ZM271,323c-15.23,6.47-25.38,27.19-31.26,41.66c-54.92,135.4,112.99,242.21,215.25,137.59c15.77-16.14,26.19-37.45,30.8-59.43C507.44,339.5,400.97,255.7,305.16,298.4C298.57,301.33,271,316.01,271,323Z"/>' +
            '<path d="M549,715c-6.33-29.98-2.8-58.17,7-87c-11.93-9.74-24.22-19.09-36-29c8.75-30.05,40.88-63.94,67-80c8.2,6.7,24.51,23.56,33.74,26.75c.66,.22,43.57-18.73,56.26-20.75c.69-6.78,2.03-40.4,5.31-44.01c12.66-13.95,87.33,.43,103.69,9.01c-1.03,15.87-4,32.17-4,48c5.6,2.84,40.32,26.85,40.59,26.87c7.85,.64,34.03-16.81,42.41-20.87c20.65,19.28,52,60.93,52,90c-14.45,7.44-28.85,14.99-43,23c5.29,27.32,4.64,60.72-8,86c5.2,2.3,35.43,23.89,37,28c4.16,10.88-48.89,79.17-61.73,82.75c-3.33,.93-10.04-5.5-12.53-7.62c-5.73-4.87-19.87-19.01-26.47-20.86c-2.81-.78-10.77,4.98-13.47,6.54C775.83,839.27,760.29,847,745,847c-.36,6.01-3.06,46.71-6,49c-1.36,1.06-9.82,1.43-11.9,1.62c-30.36,2.78-64.84-2.67-92.1-16.62c1.76-15.65,3.69-31.31,5-47c-11.59-3.48-39.29-26.62-42-27c-6.81-.95-32.33,19.93-40.66,21.78c-11.7,2.61-42.65-64.85-45.99-76c-1.18-3.94-5.16-14.14-2.35-17.78c1.6-2.07,38.19-20,40-20Zm237-94c-11.04-14.65-26.67-23.97-44.02-29.4c-97.34-30.43-169.37,78.97-106.65,159.16c9.87,12.61,26.41,22.85,41.28,28.32C772.65,814.43,851.38,698.48,786,621Z"/>' +
            '</svg>',
        options:
            '<svg class="sow-hud__action-icon" viewBox="70 135 860 775" fill="currentColor" aria-hidden="true">' +
            '<path d="M341,205c22.61-2.97,45.75-.43,68,4c5.93-17.06,20.41-35.48,30-51c30.77,6.9,63.06,23.94,88.06,42.94c9.99,7.59,18.86,16.75,28.94,24.06c-2.72,13.76-24.17,40.64-32,54c6.35,9.77,13.98,18.47,19.98,28.53C557.54,330.24,564.77,353.8,572,379c8.03,1.11,55.3,6.03,58.86,9.19c3.64,3.22,3.43,16.65,3.46,21.41c.21,39.07-5.77,74.39-18.32,111.4c-22.08-1.42-44.14-5.63-66-9c-8.44,21.72-26.54,52.27-48,63c1.83,11.12,25.68,53.57,23,60c-.82,1.97-11.84,8.03-14.18,9.49C478.2,664.88,439.36,684.31,400,684c-4.9-11.59-22-44.37-22-55c-23.77,.37-45.47-1.93-69-4c-9.82,17.51-19.87,35.3-31,52c-43.13-8.12-85.06-41.51-115-72c4-10.54,31.11-47.38,31-53c-.04-2.19-17.99-25.01-20.55-29.44C160.3,499.79,151.48,475.94,147,450c-8.18-1.7-58.86-8.15-61.62-10.63c-1.35-1.21-1.87-11.53-2.05-13.79C80.34,387.45,88.66,341.76,109,309c8.84,.39,57.25,10.02,61,7c3.88-3.13,8.56-14.9,11.67-19.63C191.1,282.04,202.55,268.7,216,258c-2.51-8.07-23.64-52.49-22-57c.65-1.79,8.44-6.25,10.43-7.63C231.08,174.97,284.22,147.01,317,148c4.84,11.37,24,47.28,24,57ZM271,323c-15.23,6.47-25.38,27.19-31.26,41.66c-54.92,135.4,112.99,242.21,215.25,137.59c15.77-16.14,26.19-37.45,30.8-59.43C507.44,339.5,400.97,255.7,305.16,298.4C298.57,301.33,271,316.01,271,323Z"/>' +
            '<path d="M549,715c-6.33-29.98-2.8-58.17,7-87c-11.93-9.74-24.22-19.09-36-29c8.75-30.05,40.88-63.94,67-80c8.2,6.7,24.51,23.56,33.74,26.75c.66,.22,43.57-18.73,56.26-20.75c.69-6.78,2.03-40.4,5.31-44.01c12.66-13.95,87.33,.43,103.69,9.01c-1.03,15.87-4,32.17-4,48c5.6,2.84,40.32,26.85,40.59,26.87c7.85,.64,34.03-16.81,42.41-20.87c20.65,19.28,52,60.93,52,90c-14.45,7.44-28.85,14.99-43,23c5.29,27.32,4.64,60.72-8,86c5.2,2.3,35.43,23.89,37,28c4.16,10.88-48.89,79.17-61.73,82.75c-3.33,.93-10.04-5.5-12.53-7.62c-5.73-4.87-19.87-19.01-26.47-20.86c-2.81-.78-10.77,4.98-13.47,6.54C775.83,839.27,760.29,847,745,847c-.36,6.01-3.06,46.71-6,49c-1.36,1.06-9.82,1.43-11.9,1.62c-30.36,2.78-64.84-2.67-92.1-16.62c1.76-15.65,3.69-31.31,5-47c-11.59-3.48-39.29-26.62-42-27c-6.81-.95-32.33,19.93-40.66,21.78c-11.7,2.61-42.65-64.85-45.99-76c-1.18-3.94-5.16-14.14-2.35-17.78c1.6-2.07,38.19-20,40-20Zm237-94c-11.04-14.65-26.67-23.97-44.02-29.4c-97.34-30.43-169.37,78.97-106.65,159.16c9.87,12.61,26.41,22.85,41.28,28.32C772.65,814.43,851.38,698.48,786,621Z"/>' +
            '</svg>',
        leave: '<path d="M13 4h6a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-6M11 16l4-4-4-4M15 12H3"/>',
        camera: '<circle cx="12" cy="12" r="7"/><circle cx="12" cy="12" r="2"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3"/>',
        tools: '<path d="M15 5a4.5 4.5 0 0 0-6 6.2L3.5 16.7a1.8 1.8 0 0 0 2.5 2.5L11.5 13.7A4.5 4.5 0 0 0 17.7 8L15 10.7l-2-2L15.7 6A4.5 4.5 0 0 0 15 5Z"/>',
        spawn: '<path d="M5 21V4m1 1h12l-3 4 3 4H6"/>',
        attack: '<path d="m5 4 14 16M19 4 5 20M3 6l3-3 3 3M15 18l3 3 3-3M3 18l3 3 3-3M15 6l3-3 3 3"/>',
        fleet: '<path d="M3 15h18l-3 5H7l-4-5ZM6 12l6-9 6 9M12 3v12M2 21c2 1 3 1 5 0s3-1 5 0 3 1 5 0 3-1 5 0"/>',
        transfer: '<path d="M4 7h15l-3-3M20 17H5l3 3M19 7l-3 3M5 17l3-3"/>',
        city: '<path d="M3 21h18M5 21V10l7-6 7 6v11M9 21v-6h6v6"/>',
        nuke: '<path d="M12 22V9M9 12l3-3 3 3M12 9l5-5M17 4h4M17 4V1M7 21h10M9 18h6"/>',
        upgrade: '<path d="m12 3 2.2 5.2L20 10l-4.4 3.5L17 19l-5-3-5 3 1.4-5.5L4 10l5.8-1.8L12 3Z"/>',
        anchor: '<path d="M12 3v12M8 7a4 4 0 1 0 8 0M4 13a8 8 0 0 0 16 0M2 13h4M18 13h4"/>',
        factory: '<path d="M3 21V9l6 3V8l6 4V5h6v16H3ZM17 8h1M17 11h1M7 17h2M12 17h2M17 17h1"/>',
        farm: '<path d="M12 21V8M12 12c-4-1-6-3-6-6 4 0 6 2 6 6ZM12 15c4-1 6-3 6-6-4 0-6 2-6 6Z"/>',
        defense: '<path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11Z"/>',
        range: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="2"/><path d="M12 2v3M22 12h-3M12 22v-3M2 12h3"/>',
        speed: '<path d="m13 2-3 8h7l-6 12 1-9H5l8-11Z"/>',
        discount: '<circle cx="7" cy="7" r="2"/><circle cx="17" cy="17" r="2"/><path d="m19 5-14 14"/>',
        clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
        construction:
            '<path d="M14.5 6.5a5 5 0 0 0-6.7 6.7L3 18l3 3 4.8-4.8a5 5 0 0 0 6.7-6.7L14 13l-3-3 3.5-3.5Z"/>',
        lock: '<rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 1 1 8 0v3"/>',
        dice: '<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M9 9h.01M15 9h.01M12 12h.01M9 15h.01M15 15h.01"/>',
        globe: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
        solo: '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
    };

    function hudIcon(name: string, cls?: string): string {
        const glyph = HUD_GLYPHS[BUILDING_GLYPHS[name] || name];
        if (glyph) {
            if (glyph.startsWith("<svg")) {
                const targetCls = cls || "sow-hud__action-icon";
                return glyph.replace('class="sow-hud__action-icon"', 'class="' + targetCls + '"');
            }
            return (
                '<svg class="' +
                (cls || "sow-hud__action-icon") +
                '" viewBox="0 0 24 24" fill="none" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><g class="sow-hud__glyph-ink">' +
                glyph +
                '</g><g class="sow-hud__glyph-face">' +
                glyph +
                "</g></svg>"
            );
        }
        if (!HUD_ICONS[name]) return "";
        return (
            '<img class="' +
            (cls || "sow-hud__action-icon") +
            '" src="' +
            asset(HUD_ICONS[name]) +
            '" alt="" aria-hidden="true">'
        );
    }

    // The menu shares the same line icons (lock, dice, settings...).
    window.SOW_hudIcon = function (name: string) {
        return hudIcon(name, "sow-menu__inline-icon");
    };

    function statusIcon(kind: string, cls?: string): string {
        const style =
            "background-image:url(&quot;" +
            escapeHudText(asset("gameplay/icons/nameplate_status_atlas.webp")) +
            "&quot;);" +
            "background-size:" +
            STATUS_CELLS.length * 100 +
            "% 100%;" +
            "background-position:" +
            (Math.max(0, STATUS_CELLS.indexOf(kind)) * 100) / (STATUS_CELLS.length - 1) +
            "% 0;";
        return (
            '<span class="' +
            (cls || "sow-hud__action-icon") +
            ' sow-hud__status-icon" style="' +
            style +
            '" aria-hidden="true"></span>'
        );
    }

    function formatEndgameExact(value: number | string | null | undefined): string {
        return String(Math.max(0, Math.trunc(Number(value) || 0)));
    }

    function formatEndgameCompact(value: number | string | null | undefined): string {
        const amount = Number(value);
        if (!Number.isFinite(amount) || amount <= 0) return "0";
        if (amount < 1000) return String(Math.round(amount));
        const suffix = amount >= 1000000 ? "M" : "K";
        const divisor = suffix === "M" ? 1000000 : 1000;
        const rounded = Math.round((amount / divisor) * 10) / 10;
        if (suffix === "K" && rounded >= 1000) return "1M";
        return String(rounded).replace(/\.0$/, "") + suffix;
    }

    function endgameStatsMarkup(prefix: string): string {
        return (
            '<div class="sow-hud__endgame-stats" id="sow-hud-' +
            prefix +
            '-stats">' +
            '      <div class="sow-hud__endgame-stat" role="group" aria-label="' +
            SOW_t("endgame.stat_kills") +
            '"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' +
            hudIcon("attack", "sow-hud__inline-icon") +
            '</span><b id="sow-hud-' +
            prefix +
            '-kills">0</b></div>' +
            '      <div class="sow-hud__endgame-stat" role="group" aria-label="' +
            SOW_t("endgame.stat_assists") +
            '"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' +
            statusIcon("allied", "sow-hud__inline-icon") +
            '</span><b id="sow-hud-' +
            prefix +
            '-assists">0</b></div>' +
            '      <div class="sow-hud__endgame-stat" role="group" aria-label="' +
            SOW_t("endgame.stat_max_territory") +
            '"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' +
            hudIcon("globe", "sow-hud__inline-icon") +
            '</span><b id="sow-hud-' +
            prefix +
            '-max-territory">0</b></div>' +
            '      <div class="sow-hud__endgame-stat" role="group" aria-label="' +
            SOW_t("endgame.stat_gold_received") +
            '"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' +
            hudIcon("gold", "sow-hud__inline-icon") +
            '</span><b id="sow-hud-' +
            prefix +
            '-gold-received">0</b></div>' +
            '      <div class="sow-hud__endgame-stat" role="group" aria-label="' +
            SOW_t("endgame.stat_troops_received") +
            '"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' +
            hudIcon("troops", "sow-hud__inline-icon") +
            '</span><b id="sow-hud-' +
            prefix +
            '-troops-received">0</b></div>' +
            "    </div>"
        );
    }

    function endgameStatRefs(prefix: string): HudEndgameRefs {
        return {
            kills: document.getElementById("sow-hud-" + prefix + "-kills"),
            assists: document.getElementById("sow-hud-" + prefix + "-assists"),
            maxTerritory: document.getElementById("sow-hud-" + prefix + "-max-territory"),
            goldReceived: document.getElementById("sow-hud-" + prefix + "-gold-received"),
            troopsReceived: document.getElementById("sow-hud-" + prefix + "-troops-received"),
        };
    }

    function renderEndgameStats(refs: HudEndgameRefs, stats: HudEndgameStats | null | undefined): void {
        stats = stats || {};
        if (refs.kills) refs.kills.textContent = formatEndgameExact(stats.kills);
        if (refs.assists) refs.assists.textContent = formatEndgameExact(stats.assists);
        if (refs.maxTerritory) refs.maxTerritory.textContent = formatEndgameCompact(stats.max_territory);
        if (refs.goldReceived) refs.goldReceived.textContent = formatEndgameCompact(stats.gold_received);
        if (refs.troopsReceived)
            refs.troopsReceived.textContent = formatEndgameCompact(stats.troops_received);
    }

    function buildingIcon(kind: string, level?: number, cls?: string): string {
        const name = String(kind || "");
        const art = window.SOW_BUILDING_ART;
        const canonical = name.charAt(0).toUpperCase() + name.slice(1).toLowerCase();
        const atlas = art && art.sprites && art.sprites[canonical];
        const sprite = atlas && atlas[String(Number(level) || 1)];
        if (
            sprite &&
            art.cell_size > 0 &&
            art.atlas_width > art.cell_size &&
            art.atlas_height > art.cell_size
        ) {
            const backgroundWidth = ((art.atlas_width * 100) / art.cell_size).toFixed(4);
            const backgroundHeight = ((art.atlas_height * 100) / art.cell_size).toFixed(4);
            const positionX = ((sprite.x * 100) / (art.atlas_width - art.cell_size)).toFixed(4);
            const positionY = ((sprite.y * 100) / (art.atlas_height - art.cell_size)).toFixed(4);
            const style =
                "background-image:url(&quot;" +
                escapeHudText(asset(art.atlas)) +
                "&quot;);" +
                "background-size:" +
                backgroundWidth +
                "% " +
                backgroundHeight +
                "%;" +
                "background-position:" +
                positionX +
                "% " +
                positionY +
                "%;";
            return (
                '<span class="' +
                (cls || "sow-hud__building-icon") +
                ' is-pixel-art" style="' +
                style +
                '" aria-hidden="true"></span>'
            );
        }
        const glyph = BUILDING_GLYPHS[name.toLowerCase()];
        return glyph ? hudIcon(glyph, cls || "sow-hud__building-icon") : "";
    }

    // Ships and nukes use the same pixel-art path as buildings, at the same size.
    function unitIcon(name: string): string {
        return (
            '<span class="sow-hud__building-icon is-pixel-art" style="background-image:url(&quot;' +
            escapeHudText(asset("gameplay/units/" + name + ".png")) +
            '&quot;);background-size:contain;" aria-hidden="true"></span>'
        );
    }

    // Gold price row under the art. Unpriced tiles keep the empty row so every tile lines up.
    function tileCost(priced: boolean): string {
        return (
            '<span class="sow-hud__tile-cost">' +
            (priced ? hudIcon("gold", "sow-hud__tile-cost-icon") + "<b></b>" : "") +
            "</span>"
        );
    }

    function tileLock() {
        return (
            '<span class="sow-hud__ring-badge" aria-hidden="true">' +
            hudIcon("lock", "sow-hud__ring-badge-icon") +
            "</span>"
        );
    }

    function setTileCost(tile: HTMLElement, cost: number): void {
        const label = tile.querySelector(".sow-hud__tile-cost b");
        const text = cost > 0 ? formatHudMetric(cost) : "";
        if (label && label.textContent !== text) label.textContent = text;
    }

    function leaderById(id: string | null | undefined): MenuLeader {
        const leaders = hudState && Array.isArray(hudState.leaders) ? hudState.leaders : [];
        const found = leaders.find(function (leader) {
            return leader.id === id;
        });
        if (found) return found;
        if (id) {
            return {
                id: id,
                name: String(id),
                slug: String(id)
                    .replace(/([a-z])([A-Z])/g, "$1_$2")
                    .replace(/\s+/g, "_")
                    .toLowerCase(),
            };
        }
        return leaders[0] || { id: "Caesar", name: "Caesar", slug: "caesar" };
    }

    function formatNameplateTroops(value: number | string | null | undefined): string {
        const count = Math.max(0, Math.floor(Number(value) || 0));
        if (count < 100000) return count.toLocaleString();
        const useMillions = count >= 999950;
        return (count / (useMillions ? 1000000 : 1000)).toFixed(1) + (useMillions ? "M" : "K");
    }

    function formatHudMetric(value: number | string | null | undefined): string {
        const amount = Number(value);
        if (!Number.isFinite(amount)) return "0";
        const abs = Math.abs(amount);
        const sign = amount < 0 ? "-" : "";
        if (abs >= 1000000) return sign + (abs / 1000000).toFixed(1) + "M";
        if (abs >= 10000) return sign + (abs / 1000).toFixed(1) + "K";
        return amount.toLocaleString(undefined, { maximumFractionDigits: 1 });
    }

    function formatHudRate(value: number | string | null | undefined): string {
        const amount = Number(value) || 0;
        return (amount > 0 ? "+" : "") + formatHudMetric(amount) + "/s";
    }

    function notificationAnchor(
        storyRoot: HTMLElement | null,
        objective: HTMLElement | null,
        nameplate: HTMLElement | null,
        gap: number,
    ):
        | { source: "objective" | "nameplate"; left: number; right: number; top: number }
        | { source: "safe"; left: null; right: null; top: null } {
        if (storyRoot && !storyRoot.hidden && objective && !objective.hidden) {
            const objectiveRect = objective.getBoundingClientRect();
            if (objectiveRect.width > 0 && objectiveRect.height > 0) {
                return {
                    source: "objective",
                    left: objectiveRect.left,
                    right: objectiveRect.right,
                    top: objectiveRect.bottom + gap,
                };
            }
        }
        if (nameplate && !nameplate.hidden) {
            const rect = nameplate.getBoundingClientRect();
            if (rect.width > 0 && rect.height > 0) {
                return { source: "nameplate", left: rect.left, right: rect.right, top: rect.bottom + gap };
            }
        }
        return { source: "safe", left: null, right: null, top: null };
    }

    function scheduleNotificationPlacement() {
        if (!hudRefs || !hudRefs.notifications) return;
        if (!window.requestAnimationFrame) {
            syncNotificationPlacement();
            return;
        }
        if (notificationLayoutFrame) return;
        notificationLayoutFrame = window.requestAnimationFrame(function () {
            notificationLayoutFrame = 0;
            syncNotificationPlacement();
        });
    }

    function observeNotificationAnchors() {
        const story = document.getElementById("sow-story");
        const objective = story && story.querySelector<HTMLElement>(".sow-story__objective");
        const nameplate = hudRefs && hudRefs.nameplate;
        if (
            story === notificationLayoutStory &&
            objective === notificationLayoutObjective &&
            nameplate === notificationLayoutNameplate
        )
            return;
        if (objective !== notificationLayoutObjective && notificationResizeObserver) {
            if (notificationLayoutObjective)
                notificationResizeObserver.unobserve(notificationLayoutObjective);
            if (objective) notificationResizeObserver.observe(objective);
        }
        if (nameplate !== notificationLayoutNameplate && notificationResizeObserver) {
            if (notificationLayoutNameplate)
                notificationResizeObserver.unobserve(notificationLayoutNameplate);
            if (nameplate) notificationResizeObserver.observe(nameplate);
        }
        notificationLayoutStory = story;
        notificationLayoutObjective = objective;
        notificationLayoutNameplate = nameplate;
        if (notificationMutationObserver) {
            // MutationObserver disconnects all targets; rebind the current anchors together.
            notificationMutationObserver.disconnect();
            if (nameplate)
                notificationMutationObserver.observe(nameplate, {
                    attributes: true,
                    attributeFilter: ["hidden"],
                });
            if (story)
                notificationMutationObserver.observe(story, {
                    childList: true,
                    attributes: true,
                    attributeFilter: ["hidden"],
                });
            if (objective) {
                notificationMutationObserver.observe(
                    objective,
                    notificationResizeObserver
                        ? { attributes: true, attributeFilter: ["hidden"] }
                        : {
                              attributes: true,
                              attributeFilter: ["hidden"],
                              childList: true,
                              subtree: true,
                              characterData: true,
                          },
                );
            }
        }
    }

    function observeNotificationLayout() {
        if (!hudRefs || !hudRefs.nameplate || !hudRefs.notifications) return;
        if (notificationLayoutNotifications !== hudRefs.notifications) {
            notificationLayoutNotifications = hudRefs.notifications;
            notificationLayoutState = "";
        }
        if (!notificationResizeObserver && typeof window.ResizeObserver === "function") {
            notificationResizeObserver = new window.ResizeObserver(function () {
                syncCompactHudBounds();
                syncNotificationPlacement();
            });
        }
        if (!notificationMutationObserver && typeof window.MutationObserver === "function") {
            notificationMutationObserver = new window.MutationObserver(function () {
                observeNotificationAnchors();
                syncCompactHudDensity();
                scheduleNotificationPlacement();
            });
        }
        [hudRefs.topbar, hudRefs.nameplate, hudRefs.dock, hudRefs.dockStack].forEach(function (element) {
            if (element && notificationResizeObserver) notificationResizeObserver.observe(element);
        });
        observeNotificationAnchors();
        syncCompactHudBounds();
        if (!notificationLayoutStarted) {
            notificationLayoutStarted = true;
            const onViewportChange = function () {
                const hudScreen = measureHudScreen();
                syncCompactHudBounds(hudScreen);
                renderNotifications([], false, hudScreen);
                scheduleNotificationPlacement();
            };
            window.addEventListener("resize", onViewportChange, { passive: true });
            if (window.visualViewport) {
                window.visualViewport.addEventListener("resize", onViewportChange, { passive: true });
                window.visualViewport.addEventListener("scroll", onViewportChange, { passive: true });
            }
            document.addEventListener(
                "focusout",
                function () {
                    if (window.requestAnimationFrame)
                        window.requestAnimationFrame(function () {
                            syncCompactHudBounds();
                        });
                    else syncCompactHudBounds();
                },
                { passive: true },
            );
            window.addEventListener("sow:locale-change", scheduleNotificationPlacement);
        }
        scheduleNotificationPlacement();
    }

    function syncNotificationPlacement() {
        if (!hudRefs || !hudRefs.notifications) return;
        const notifications = hudRefs.notifications;
        const story = document.getElementById("sow-story");
        const objective = story && story.querySelector<HTMLElement>(".sow-story__objective");
        const anchor = notificationAnchor(story, objective, hudRefs.nameplate, 12);
        let inlineStart = null;
        if (anchor.source !== "safe") {
            const direction = window.getComputedStyle
                ? window.getComputedStyle(hudRoot!).direction
                : document.documentElement.dir;
            inlineStart = direction === "rtl" ? window.innerWidth - anchor.right : anchor.left;
            const fallbackWidth = Math.min(
                window.innerWidth <= 720 ? 300 : 360,
                Math.max(0, window.innerWidth - 32),
            );
            const notificationWidth = notifications.offsetWidth || fallbackWidth;
            inlineStart = Math.max(8, Math.min(inlineStart, window.innerWidth - notificationWidth - 8));
        }
        const topValue = anchor.source === "safe" ? null : Math.round(Math.max(0, anchor.top));
        const inlineStartValue = inlineStart == null ? null : Math.round(inlineStart);
        const state = [anchor.source, topValue, inlineStartValue].join(":");
        if (state === notificationLayoutState) return;
        notificationLayoutState = state;
        if (topValue == null) notifications.style.removeProperty("--sow-hud-notifications-top");
        else notifications.style.setProperty("--sow-hud-notifications-top", topValue + "px");
        if (inlineStartValue == null)
            notifications.style.removeProperty("--sow-hud-notifications-inline-start");
        else notifications.style.setProperty("--sow-hud-notifications-inline-start", inlineStartValue + "px");
        notifications.classList.toggle("is-positioned", inlineStartValue != null);
        const hasVisibleNotification = notificationCards.some(function (parts) {
            return parts && parts.card && !parts.card.hidden;
        });
        if (hasVisibleNotification && !hudRoot!.classList.contains("is-reduced-motion")) {
            notifications.classList.remove("is-reanchoring");
            void notifications.offsetWidth;
            notifications.classList.add("is-reanchoring");
        } else {
            notifications.classList.remove("is-reanchoring");
        }
    }

    function createHudRefs(): HudRefs {
        const root = hudRoot!;
        return {
            fps: requiredHudElement("sow-hud-fps"),
            ping: requiredHudElement("sow-hud-ping"),
            diagnostics: requiredHudQuery(root, ".sow-hud__diagnostics"),
            nameplate: requiredHudElement("sow-hud-nameplate"),
            plateAvatar: requiredHudImage("sow-hud-nameplate-avatar"),
            plateName: requiredHudElement("sow-hud-nameplate-name"),
            plateFill: requiredHudElement("sow-hud-nameplate-fill"),
            plateTroops: requiredHudElement("sow-hud-nameplate-troops"),
            plateGold: requiredHudElement("sow-hud-nameplate-gold"),
            plateGoldWrap: requiredHudQuery(root, ".sow-hud__nameplate-stat--gold"),
            plateGoldRate: requiredHudElement("sow-hud-nameplate-gold-rate"),
            plateTroopRate: requiredHudElement("sow-hud-nameplate-troop-rate"),
            inboxCount: requiredHudElement("sow-hud-inbox-count"),
            hoverCard: requiredHudElement("sow-hud-hover-card"),
            hoverName: requiredHudElement("sow-hud-hover-name"),
            hoverPct: requiredHudElement("sow-hud-hover-pct"),
            hoverTroops: requiredHudElement("sow-hud-hover-troops"),
            hoverGold: requiredHudElement("sow-hud-hover-gold"),
            hoverBlds: requiredHudElement("sow-hud-hover-blds"),
            leftRail: requiredHudElement("sow-hud-left-rail"),
            slider: requiredHudInput("sow-hud-slider"),
            armyAllocation: requiredHudElement("sow-hud-army-allocation"),
            armyRatio: requiredHudElement("sow-hud-army-ratio"),
            rightRail: requiredHudElement("sow-hud-right-rail"),
            devSidebar: requiredHudElement("sow-hud-dev-sidebar"),
            devBtn: requiredHudElement("sow-hud-dev-btn"),
            settings: requiredHudElement("sow-hud-settings"),
            deployBtn: requiredHudElement("sow-hud-deploy-btn"),
            deployTimer: requiredHudElement("sow-hud-deploy-timer"),
            buildingsStrip: requiredHudElement("sow-hud-buildings-strip"),
            worldProgressOverlay: requiredHudElement("sow-hud-world-progress-overlay"),
            worldCelebrationsOverlay: requiredHudElement("sow-hud-world-celebrations-overlay"),
            buildingButtons: Array.from(
                root.querySelectorAll<HTMLButtonElement>("[data-command='select_building']"),
            ),
            fleetStrip: requiredHudElement("sow-hud-fleet-strip"),
            fleetTrade: requiredHudQuery(root, "[data-fleet-kind='trade']"),
            fleetWarship: requiredHudButtonQuery(root, "[data-fleet-kind='warship']"),
            fleetNuke: requiredHudButtonQuery(root, "[data-fleet-kind='nuke']"),
            panelButtons: Array.from(
                root.querySelectorAll<HTMLButtonElement>(".sow-hud__topbar [data-command^='toggle_']"),
            ),
            leaderboard: requiredHudElement("sow-hud-leaderboard"),
            rows: requiredHudElement("sow-hud-lb-rows"),
            lbSearch: requiredHudInput("sow-hud-lb-search"),
            lbEmpty: requiredHudElement("sow-hud-lb-empty"),
            inbox: requiredHudElement("sow-hud-inbox"),
            inboxRows: requiredHudElement("sow-hud-inbox-rows"),
            inboxClose: requiredHudElement("sow-hud-inbox-close"),
            inboxButton: requiredHudQuery(root, ".sow-hud__inbox-pill"),
            inboxTotal: requiredHudElement("sow-hud-inbox-total"),
            inboxEmpty: requiredHudElement("sow-hud-inbox-empty"),
            inboxRequests: Object.create(null),
            inboxRequestKeys: [],
            inboxRenderKey: "",
            transfer: requiredHudElement("sow-hud-transfer"),
            transferTarget: requiredHudElement("sow-hud-transfer-target"),
            transferClose: requiredHudElement("sow-hud-transfer-close"),
            transferReturnFocus: null,
            transferGold: requiredHudInput("sow-hud-transfer-gold"),
            transferTroops: requiredHudInput("sow-hud-transfer-troops"),
            transferIdentity: createPlayerIdentity(),
            betrayal: requiredHudElement("sow-hud-betrayal-modal"),
            betrayalTarget: requiredHudElement("sow-hud-betrayal-target"),
            betrayalCancel: requiredHudElement("sow-hud-betrayal-cancel"),
            betrayalReturnFocus: null,
            betrayalIdentity: createPlayerIdentity(),
            surrender: requiredHudElement("sow-hud-surrender-modal"),
            surrenderTitle: requiredHudElement("sow-hud-surrender-title"),
            surrenderDesc: requiredHudElement("sow-hud-surrender-desc"),
            surrenderStats: requiredHudElement("sow-hud-surrender-stats"),
            surrenderStatValues: endgameStatRefs("surrender"),
            surrenderSaveNote: requiredHudElement("sow-hud-surrender-save-note"),
            surrenderPortrait: requiredHudImage("sow-hud-surrender-portrait"),
            surrenderCancel: requiredHudElement("sow-hud-surrender-cancel"),
            surrenderActionLabel: requiredHudElement("sow-hud-surrender-action-label"),
            endgame: requiredHudElement("sow-hud-endgame-modal"),
            endgameCard: requiredHudQuery(document, "#sow-hud-endgame-modal .sow-hud__endgame-card"),
            endgameBanner: requiredHudElement("sow-hud-endgame-banner"),
            endgamePortrait: requiredHudImage("sow-hud-endgame-portrait"),
            endgameStatValues: endgameStatRefs("endgame"),
            endgameStore: requiredHudElement("sow-hud-endgame-store"),
            endgameStoreName: requiredHudElement("sow-hud-endgame-store-name"),
            endgameStoreCopy: requiredHudElement("sow-hud-endgame-store-copy"),
            endgameStoreAction: requiredHudElement("sow-hud-endgame-store-action"),
            endgameObserve: requiredHudButton("sow-hud-endgame-observe"),
            mapMenu: requiredHudElement("sow-hud-map-menu"),
            placementOverlay: requiredHudElement("sow-hud-placement-overlay"),
            placementPanel: requiredHudElement("sow-hud-placement-panel"),
            placementArrows: {
                up: requiredHudButtonQuery(root, "#sow-hud-placement-overlay [data-placement-nudge='up']"),
                right: requiredHudButtonQuery(
                    root,
                    "#sow-hud-placement-overlay [data-placement-nudge='right']",
                ),
                down: requiredHudButtonQuery(
                    root,
                    "#sow-hud-placement-overlay [data-placement-nudge='down']",
                ),
                left: requiredHudButtonQuery(
                    root,
                    "#sow-hud-placement-overlay [data-placement-nudge='left']",
                ),
            },
            operations: requiredHudElement("sow-hud-operations"),
            operationRows: requiredHudElement("sow-hud-operations-list"),
            combatOperationRows: Object.create(null),
            combatOperationsOrderKey: "",
            notifications: document.createElement("div"),
            topbar: requiredHudQuery(root, ".sow-hud__topbar"),
            dock: requiredHudElement("sow-hud-dock"),
            dockStack: requiredHudElement("sow-hud-dock-stack"),
            mapFeedback: document.createElement("div"),
        };
    }

    function ensureHudDom() {
        if (!hudRoot || hudInitialized) return;
        hudInitialized = true;
        emoteTextKey = "";
        const initialMusicVol =
            hudState && hudState.settings && hudState.settings.music_volume != null
                ? Number(hudState.settings.music_volume)
                : 0.8;
        const initialMusicPct = Math.round(Math.max(0, Math.min(1, initialMusicVol)) * 100);
        hudRoot.innerHTML =
            "" +
            '<header class="sow-hud__topbar">' +
            '  <div class="sow-hud__status-right">' +
            '    <button class="sow-hud__icon-btn sow-hud__exit-pill" type="button" data-command="prompt_surrender" aria-label="' +
            SOW_t("hud.leave_match") +
            '" title="' +
            SOW_t("hud.leave_match") +
            '">' +
            hudIcon("leave", "sow-hud__action-icon") +
            "</button>" +
            '    <button class="sow-hud__icon-btn" type="button" data-command="toggle_settings" aria-label="' +
            SOW_t("menu.settings") +
            '" title="' +
            SOW_t("menu.settings") +
            '">' +
            hudIcon("settings", "sow-hud__action-icon") +
            "</button>" +
            '    <button class="sow-hud__icon-btn sow-hud__inbox-pill" type="button" data-command="toggle_inbox" aria-label="' +
            SOW_t("hud.inbox") +
            '" title="' +
            SOW_t("hud.inbox") +
            '">' +
            hudIcon("inbox") +
            '<span class="sow-hud__inbox-badge" id="sow-hud-inbox-count" hidden>0</span></button>' +
            '    <button class="sow-hud__icon-btn" type="button" data-command="toggle_leaderboard" aria-label="' +
            SOW_t("hud.rankings") +
            '" title="' +
            SOW_t("hud.rankings") +
            '">' +
            hudIcon("rankings", "sow-hud__action-icon") +
            "</button>" +
            '    <button class="sow-hud__icon-btn hidden" type="button" data-command="toggle_dev_sidebar" id="sow-hud-dev-btn" hidden aria-label="' +
            SOW_t("hud.dev_tools") +
            '" title="' +
            SOW_t("hud.dev_tools") +
            '">' +
            hudIcon("tools", "sow-hud__action-icon") +
            "</button>" +
            '    <div class="sow-hud__diagnostics" id="sow-hud-diagnostics">' +
            '      <span class="sow-hud__fps" id="sow-hud-fps">' +
            SOW_t("hud.fps", { fps: "--" }) +
            "</span>" +
            '      <span class="sow-hud__ping" id="sow-hud-ping"></span>' +
            "    </div>" +
            "  </div>" +
            "</header>" +
            '<div class="sow-hud__nameplate" id="sow-hud-nameplate" data-command="center_camera" role="button" tabindex="0" aria-label="' +
            SOW_t("hud.center_camera") +
            '" hidden>' +
            '  <img class="sow-hud__nameplate-avatar" id="sow-hud-nameplate-avatar" alt="" draggable="false">' +
            '  <div class="sow-hud__nameplate-copy"><strong id="sow-hud-nameplate-name"></strong>' +
            '  <div class="sow-hud__nameplate-bar"><i id="sow-hud-nameplate-fill"></i><span class="sow-hud__nameplate-readout"><span id="sow-hud-nameplate-troops"></span>' +
            hudIcon("troops", "sow-hud__inline-icon") +
            "</span></div>" +
            '  <div class="sow-hud__nameplate-economy">' +
            '    <span class="sow-hud__nameplate-stat sow-hud__nameplate-stat--gold" title="' +
            SOW_t("hud.gold_treasury") +
            '" aria-label="' +
            SOW_t("hud.gold_treasury") +
            '"><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' +
            currencyAsset("gold") +
            '" alt="" aria-hidden="true"><b id="sow-hud-nameplate-gold">0</b></span>' +
            '    <span class="sow-hud__nameplate-stat sow-hud__nameplate-stat--gold-rate" title="' +
            SOW_t("hud.gold_net_rate") +
            '" aria-label="' +
            SOW_t("hud.gold_net_rate") +
            '"><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' +
            currencyAsset("gold") +
            '" alt="" aria-hidden="true"><b id="sow-hud-nameplate-gold-rate">0/s</b></span>' +
            '    <span class="sow-hud__nameplate-stat sow-hud__nameplate-stat--troop-rate" title="' +
            SOW_t("hud.troop_production_rate") +
            '" aria-label="' +
            SOW_t("hud.troop_production_rate") +
            '">' +
            hudIcon("troops", "sow-hud__inline-icon") +
            '<b id="sow-hud-nameplate-troop-rate">0/s</b></span>' +
            "  </div></div>" +
            "</div>" +
            '<div class="sow-hud__hover-card hidden" id="sow-hud-hover-card">' +
            '  <div class="sow-hud__hover-header"><b id="sow-hud-hover-name">' +
            SOW_t("hud.territory") +
            "</b></div>" +
            '  <div class="sow-hud__hover-stats">' +
            '    <span><b id="sow-hud-hover-pct">0%</b> ' +
            SOW_t("hud.land") +
            "</span>" +
            '    <span><b id="sow-hud-hover-troops">0</b> ' +
            hudIcon("troops", "sow-hud__inline-icon") +
            "</span>" +
            '    <span><b id="sow-hud-hover-gold">0</b> <img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' +
            currencyAsset("gold") +
            '" alt="" aria-hidden="true"></span>' +
            "  </div>" +
            '  <div class="sow-hud__hover-buildings" id="sow-hud-hover-blds"></div>' +
            "</div>" +
            '<aside class="sow-hud__left-rail" id="sow-hud-left-rail">' +
            '  <div class="sow-hud__rail-card">' +
            '    <div class="sow-hud__slider-vertical-wrap">' +
            '      <input type="range" min="5" max="100" value="50" step="5" class="sow-hud__range-vertical" id="sow-hud-slider" aria-label="' +
            SOW_t("hud.map_action_attack") +
            '">' +
            "    </div>" +
            '    <div class="sow-hud__army-allocation">' +
            '      <strong id="sow-hud-army-ratio" aria-hidden="true">50%</strong>' +
            "    </div>" +
            "  </div>" +
            '  <output class="sow-hud__allocation-detail" id="sow-hud-army-allocation" for="sow-hud-slider" aria-hidden="true"></output>' +
            "</aside>" +
            '<aside class="sow-hud__right-rail" id="sow-hud-right-rail">' +
            '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_in" aria-label="' +
            SOW_t("hud.zoom_in") +
            '" title="' +
            SOW_t("hud.zoom_in") +
            '">' +
            hudIcon("plus") +
            "</button>" +
            '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_out" aria-label="' +
            SOW_t("hud.zoom_out") +
            '" title="' +
            SOW_t("hud.zoom_out") +
            '">' +
            hudIcon("minus") +
            "</button>" +
            '  <button type="button" class="sow-hud__icon-btn" data-command="center_camera" aria-label="' +
            SOW_t("hud.center_camera") +
            '" title="' +
            SOW_t("hud.center_camera") +
            '">' +
            hudIcon("home", "sow-hud__action-icon") +
            "</button>" +
            "</aside>" +
            '<aside class="sow-hud__sheet sow-hud__dev-sidebar hidden" id="sow-hud-dev-sidebar">' +
            '  <div class="sow-hud__sheet-header"><b class="sow-hud__sheet-title">' +
            SOW_t("hud.dev_tools") +
            "</b>" +
            '    <button type="button" class="sow-hud__sheet-btn" data-command="dev_menu" aria-label="More" aria-expanded="false">⋯</button>' +
            '    <button type="button" class="sow-hud__sheet-btn" data-command="toggle_dev_sidebar" aria-label="' +
            SOW_t("hud.close_developer_tools") +
            '">✕</button>' +
            '    <div class="sow-hud__dev-menu" id="sow-hud-dev-menu" hidden>' +
            '      <button type="button" data-command="dev_vfx_all" data-on="1">All effects on</button>' +
            '      <button type="button" data-command="dev_vfx_all" data-on="0">All effects off</button>' +
            '      <button type="button" data-command="dev_reset_all" id="sow-hud-dev-reset-all">Reset all</button>' +
            "    </div>" +
            "  </div>" +
            '  <nav class="sow-hud__dev-tabs" id="sow-hud-dev-tabs" aria-label="Groups"></nav>' +
            '  <div class="sow-hud__dev-transport" id="sow-hud-dev-transport" hidden>' +
            '    <input type="range" min="0" max="1" step="0.01" data-dev-preview="phase" aria-label="Preview position">' +
            '    <label class="sow-hud__dev-chip"><input type="checkbox" data-dev-preview="paused"><span>Pause</span></label>' +
            '    <label class="sow-hud__dev-chip"><input type="checkbox" data-dev-preview="repeat"><span>Loop</span></label>' +
            "  </div>" +
            '  <div class="sow-hud__sheet-body sow-hud__dev-body">' +
            '    <input class="sow-hud__dev-search" type="search" data-dev-search placeholder="Search" aria-label="Find shader setting">' +
            '    <div class="sow-hud__dev-perf" id="sow-hud-dev-perf" role="group" aria-label="Frame cost switches" hidden></div>' +
            '    <div class="sow-hud__dev-controls" id="sow-hud-dev-controls"></div>' +
            "  </div>" +
            "</aside>" +
            '<aside class="sow-hud__panel sow-hud__settings sow-hud__sheet hidden" id="sow-hud-settings">' +
            '  <div class="sow-hud__sheet-header">' +
            '    <div class="sow-hud__sheet-title-group">' +
            '      <span class="sow-hud__sheet-title-badge" aria-hidden="true">' +
            hudIcon("settings", "sow-hud__sheet-title-icon") +
            "      </span>" +
            '      <h3 class="sow-hud__sheet-title">' +
            SOW_t("menu.settings") +
            "      </h3>" +
            "    </div>" +
            '    <button class="sow-hud__close-btn sow-hud__sheet-btn" type="button" data-command="toggle_settings" aria-label="' +
            SOW_t("hud.close_settings") +
            '">' +
            hudIcon("close", "sow-hud__close-icon") +
            "</button>" +
            "  </div>" +
            '  <div class="sow-hud__sheet-body sow-hud__settings-body">' +
            '    <label class="sow-hud__setting-row sow-hud__setting-card"><span>' +
            SOW_t("hud.sound") +
            '</span><input class="sow-hud__switch" type="checkbox" data-hud-setting="mute_all"></label>' +
            '    <div class="sow-hud__setting-row sow-hud__setting-card sow-hud__setting-card--slider">' +
            '      <div class="sow-hud__setting-slider-header"><span>' +
            SOW_t("hud.music") +
            '</span><span class="sow-hud__setting-value" id="sow-hud-music-value">' +
            initialMusicPct +
            "%</span></div>" +
            '      <input type="range" min="0" max="1" step="0.01" value="' +
            initialMusicVol +
            '" data-hud-setting="music_volume" style="--sow-music-fill: ' +
            initialMusicPct +
            '%;" aria-label="' +
            SOW_t("hud.music") +
            '">' +
            "    </div>" +
            '    <label class="sow-hud__setting-row sow-hud__setting-card"><span>' +
            SOW_t("hud.reduced_motion") +
            '</span><input class="sow-hud__switch" type="checkbox" data-hud-setting="reduced_motion"></label>' +
            '    <label class="sow-hud__setting-row sow-hud__setting-card"><span>' +
            SOW_t("hud.free_zoom_out") +
            '</span><input class="sow-hud__switch" type="checkbox" data-hud-setting="free_zoom_out"></label>' +
            '    <label class="sow-hud__setting-row sow-hud__setting-card"><span>' +
            SOW_t("hud.show_fps_ping") +
            '</span><input class="sow-hud__switch" type="checkbox" data-hud-setting="show_fps_ping"></label>' +
            (hudState && hudState.settings && hudState.settings.show_dev_tools_available
                ? '    <label class="sow-hud__setting-row sow-hud__setting-card"><span>' +
                  SOW_t("hud.show_dev_tools") +
                  '</span><input class="sow-hud__switch" type="checkbox" data-hud-setting="show_dev_tools"></label>'
                : "") +
            renderLocaleSettingRow("hud-settings-language") +
            "  </div>" +
            "</aside>" +
            '<div class="sow-hud__dock-stack" id="sow-hud-dock-stack">' +
            '<footer class="sow-hud__dock" id="sow-hud-dock">' +
            '  <div class="sow-hud__dock-inner" id="sow-hud-dock-inner">' +
            '    <div class="sow-hud__dock-actions">' +
            '      <div class="sow-hud__deploy-panel hidden" id="sow-hud-deploy-btn">' +
            '        <span class="sow-hud__deploy-title">' +
            SOW_t("hud.choose_spawn") +
            "</span>" +
            '        <b class="sow-hud__deploy-timer" id="sow-hud-deploy-timer">' +
            SOW_t("hud.ready") +
            "</b>" +
            "      </div>" +
            '      <div class="sow-hud__buildings-strip" id="sow-hud-buildings-strip">' +
            '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="City" aria-label="' +
            SOW_t("hud.map_action_city") +
            '">' +
            buildingIcon("City") +
            tileCost(true) +
            "</button>" +
            '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Factory" aria-label="' +
            SOW_t("hud.map_action_factory") +
            '">' +
            buildingIcon("Factory") +
            tileCost(true) +
            "</button>" +
            '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Port" aria-label="' +
            SOW_t("hud.map_action_port") +
            '">' +
            buildingIcon("Port") +
            tileCost(true) +
            "</button>" +
            '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Tower" aria-label="' +
            SOW_t("hud.map_action_tower") +
            '">' +
            buildingIcon("Tower") +
            tileCost(true) +
            "</button>" +
            '        <button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="Farm" aria-label="Farm">' +
            buildingIcon("Farm") +
            tileCost(true) +
            "</button>" +
            "      </div>" +
            '      <div class="sow-hud__fleet-strip" id="sow-hud-fleet-strip">' +
            '        <span class="sow-hud__building-btn sow-hud__fleet-status" data-fleet-kind="trade" role="img" aria-label="' +
            SOW_t("hud.map_action_fleet") +
            '">' +
            unitIcon("trade-ship") +
            tileCost(false) +
            tileLock() +
            "</span>" +
            '        <button type="button" class="sow-hud__building-btn" data-command="select_warship" data-fleet-kind="warship" aria-label="' +
            SOW_t("hud.map_action_build_warship") +
            '" title="' +
            SOW_t("hud.map_action_build_warship") +
            '">' +
            unitIcon("warship") +
            tileCost(true) +
            tileLock() +
            "</button>" +
            '        <button type="button" class="sow-hud__building-btn" data-command="select_nuke" data-fleet-kind="nuke" aria-label="' +
            SOW_t("hud.map_action_nuke") +
            '" title="' +
            SOW_t("hud.map_action_nuke") +
            '">' +
            unitIcon("atom-bomb") +
            tileCost(true) +
            tileLock() +
            "</button>" +
            "      </div>" +
            "    </div>" +
            "  </div>" +
            "</footer>" +
            '<aside class="sow-hud__operations" id="sow-hud-operations" hidden aria-label="' +
            SOW_t("hud.combat_operations") +
            '">' +
            '  <div class="sow-hud__operations-list" id="sow-hud-operations-list"></div>' +
            "</aside>" +
            "</div>" +
            '<aside class="sow-hud__panel sow-hud__leaderboard sow-hud__sheet hidden" id="sow-hud-leaderboard">' +
            '  <div class="sow-hud__sheet-header sow-hud__leaderboard-header">' +
            '    <h3 class="sow-hud__sheet-title">' +
            hudIcon("rankings", "sow-hud__inline-icon") +
            " " +
            SOW_t("hud.rankings_title") +
            "</h3>" +
            '    <div class="sow-hud__leaderboard-search-wrap">' +
            '      <span class="sow-hud__leaderboard-search-icon" aria-hidden="true">' +
            hudIcon("search", "sow-hud__inline-icon") +
            "</span>" +
            '      <input class="sow-hud__leaderboard-search" id="sow-hud-lb-search" type="search" placeholder="' +
            SOW_t("hud.search_rankings") +
            '" aria-label="' +
            SOW_t("hud.search_rankings") +
            '" autocomplete="off" spellcheck="false" />' +
            "    </div>" +
            '    <button class="sow-hud__close-btn sow-hud__sheet-btn" type="button" data-command="toggle_leaderboard" aria-label="' +
            SOW_t("hud.close_rankings") +
            '">✕</button>' +
            "  </div>" +
            '  <div class="sow-hud__sheet-body sow-hud__leaderboard-rows" id="sow-hud-lb-rows"></div>' +
            '  <p class="sow-hud__leaderboard-empty" id="sow-hud-lb-empty" hidden>' +
            SOW_t("hud.no_rankings_found") +
            "</p>" +
            "</aside>" +
            '<aside class="sow-hud__inbox sow-hud__sheet hidden" id="sow-hud-inbox" role="dialog" aria-modal="false" aria-labelledby="sow-hud-inbox-title">' +
            '  <div class="sow-hud__sheet-header sow-hud__inbox-header"><span class="sow-hud__transfer-icon" aria-hidden="true">' +
            hudIcon("inbox", "sow-hud__inline-icon") +
            '</span><h3 class="sow-hud__sheet-title" id="sow-hud-inbox-title">' +
            SOW_t("hud.inbox") +
            '</h3><span class="sow-hud__inbox-total" id="sow-hud-inbox-total" aria-hidden="true">0</span><button class="sow-hud__close-btn sow-hud__sheet-btn" id="sow-hud-inbox-close" type="button" data-command="toggle_inbox" aria-label="' +
            SOW_t("hud.close_inbox") +
            '">✕</button></div>' +
            '  <div class="sow-hud__sheet-body sow-hud__inbox-list" id="sow-hud-inbox-rows"></div>' +
            '  <p class="sow-hud__inbox-empty" id="sow-hud-inbox-empty" hidden>' +
            SOW_t("hud.no_pending_requests") +
            "</p>" +
            "</aside>" +
            '<aside class="sow-hud__dialog sow-hud__transfer hidden" id="sow-hud-transfer" role="dialog" aria-modal="false" aria-labelledby="sow-hud-transfer-title">' +
            '  <div class="sow-hud__transfer-header"><span class="sow-hud__transfer-icon" aria-hidden="true">' +
            hudIcon("transfer", "sow-hud__inline-icon") +
            '</span><h3 id="sow-hud-transfer-title">' +
            SOW_t("hud.resource_transfer") +
            '</h3><button class="sow-hud__close-btn" id="sow-hud-transfer-close" type="button" data-command="close_transfer" aria-label="' +
            SOW_t("hud.close_resource_transfer") +
            '">✕</button></div>' +
            '  <div class="sow-hud__dialog-body">' +
            '  <div class="sow-hud__dialog-participant" id="sow-hud-transfer-target"></div>' +
            '  <div class="sow-hud__transfer-fields">' +
            "    <label><span>" +
            hudIcon("troops", "sow-hud__inline-icon") +
            SOW_t("hud.troops") +
            '</span><input id="sow-hud-transfer-troops" type="number" inputmode="numeric" min="0" step="1" value="0"></label>' +
            '    <label><span><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' +
            currencyAsset("gold") +
            '" alt="">' +
            SOW_t("hud.gold") +
            '</span><input id="sow-hud-transfer-gold" type="number" inputmode="numeric" min="0" step="1" value="0"></label>' +
            "  </div>" +
            "  </div>" +
            '  <div class="sow-hud__transfer-actions"><button class="sow-hud__transfer-send" type="button" data-command="send_resources"><span>' +
            SOW_t("hud.send") +
            '</span><span aria-hidden="true">→</span></button><button class="sow-hud__transfer-request" type="button" data-command="request_resources">' +
            SOW_t("hud.request") +
            "</button></div>" +
            "</aside>" +
            '<div class="sow-hud__modal-backdrop sow-hud__betrayal-backdrop hidden" id="sow-hud-betrayal-modal">' +
            '  <section class="sow-hud__dialog sow-hud__betrayal" role="dialog" aria-modal="true" aria-labelledby="sow-hud-betrayal-title" aria-describedby="sow-hud-betrayal-copy">' +
            '    <div class="sow-hud__transfer-header"><span class="sow-hud__transfer-icon sow-hud__betrayal-mark" aria-hidden="true">' +
            statusIcon("traitor", "sow-hud__inline-icon") +
            '</span><h3 id="sow-hud-betrayal-title">' +
            SOW_t("hud.break_alliance") +
            '</h3><button class="sow-hud__close-btn" id="sow-hud-betrayal-close" type="button" data-command="cancel_betrayal" aria-label="' +
            SOW_t("endgame.cancel") +
            '">✕</button></div>' +
            '    <div class="sow-hud__dialog-body"><div class="sow-hud__dialog-participant" id="sow-hud-betrayal-target"></div><p class="sow-hud__betrayal-copy" id="sow-hud-betrayal-copy">' +
            SOW_t("hud.break_alliance_body") +
            "</p></div>" +
            '    <div class="sow-hud__transfer-actions sow-hud__betrayal-actions"><button class="sow-hud__transfer-request" id="sow-hud-betrayal-cancel" type="button" data-command="cancel_betrayal">' +
            SOW_t("hud.keep_alliance") +
            '</button><button class="sow-hud__betrayal-confirm" id="sow-hud-betrayal-confirm" type="button" data-command="confirm_betrayal">' +
            statusIcon("traitor", "sow-hud__inline-icon") +
            SOW_t("hud.attack") +
            "</button></div>" +
            "  </section>" +
            "</div>" +
            '<div class="sow-hud__endgame-backdrop hidden" id="sow-hud-surrender-modal">' +
            '  <section class="sow-hud__endgame-card sow-hud__exit-card" data-result="defeat" role="dialog" aria-modal="true" aria-labelledby="sow-hud-surrender-banner" aria-describedby="sow-hud-surrender-desc">' +
            '    <div class="sow-hud__endgame-hero">' +
            '      <div class="sow-hud__endgame-portrait-wrap">' +
            '        <img class="sow-hud__endgame-portrait" id="sow-hud-surrender-portrait" src="" alt="' +
            SOW_t("hud.leader_avatar") +
            '" />' +
            "      </div>" +
            '      <div class="sow-hud__endgame-hero-copy">' +
            '        <h2 class="sow-hud__endgame-banner" id="sow-hud-surrender-banner">' +
            hudIcon("surrender", "sow-hud__inline-icon") +
            ' <span id="sow-hud-surrender-title">' +
            SOW_t("endgame.leave_match") +
            "</span></h2>" +
            '        <p class="sow-hud__endgame-desc" id="sow-hud-surrender-desc">' +
            SOW_t("endgame.your_battle_will_end") +
            "</p>" +
            "      </div>" +
            "    </div>" +
            "    " +
            endgameStatsMarkup("surrender") +
            '    <p class="sow-hud__endgame-desc" id="sow-hud-surrender-save-note">' +
            SOW_t("endgame.leave_rewards_saved") +
            "</p>" +
            '    <div class="sow-hud__endgame-actions">' +
            '      <button class="sow-hud__endgame-secondary" type="button" data-command="close_surrender_modal" id="sow-hud-surrender-cancel">' +
            SOW_t("endgame.cancel") +
            "</button>" +
            '      <button class="sow-hud__endgame-primary" type="button" data-command="confirm_surrender"><span aria-hidden="true">⌂</span> <span id="sow-hud-surrender-action-label">' +
            SOW_t("endgame.leave_match") +
            "</span></button>" +
            "    </div>" +
            "  </section>" +
            "</div>" +
            '<div class="sow-hud__endgame-backdrop hidden" id="sow-hud-endgame-modal">' +
            '  <section class="sow-hud__endgame-card" role="dialog" aria-modal="true" aria-labelledby="sow-hud-endgame-banner">' +
            '    <div class="sow-hud__endgame-hero">' +
            '      <div class="sow-hud__endgame-portrait-wrap">' +
            '        <img class="sow-hud__endgame-portrait" id="sow-hud-endgame-portrait" src="" alt="' +
            SOW_t("hud.leader_avatar") +
            '" />' +
            "      </div>" +
            '      <div class="sow-hud__endgame-hero-copy">' +
            '        <h2 class="sow-hud__endgame-banner" id="sow-hud-endgame-banner">' +
            SOW_t("hud.defeat") +
            "</h2>" +
            "      </div>" +
            "    </div>" +
            "    " +
            endgameStatsMarkup("endgame") +
            '    <div class="sow-hud__endgame-store" id="sow-hud-endgame-store" aria-label="' +
            SOW_t("store.featured_skin") +
            '">' +
            '      <span class="sow-hud__endgame-store-icon" aria-hidden="true">✦</span>' +
            '      <span class="sow-hud__endgame-store-copy"><b id="sow-hud-endgame-store-name">' +
            SOW_t("store.featured_skin") +
            '</b><small id="sow-hud-endgame-store-copy">' +
            SOW_t("store.original_cosmetic") +
            "</small></span>" +
            '      <button class="sow-hud__endgame-store-state" type="button" data-command="open_store" id="sow-hud-endgame-store-action">' +
            SOW_t("hud.view_shop") +
            "</button>" +
            "    </div>" +
            '    <div class="sow-hud__endgame-actions">' +
            '      <button class="sow-hud__endgame-secondary hidden" type="button" data-command="continue_observing" id="sow-hud-endgame-observe"><span aria-hidden="true">◉</span> ' +
            SOW_t("hud.continue_observing") +
            "</button>" +
            '      <button class="sow-hud__endgame-primary" type="button" data-command="confirm_endgame_leave"><span aria-hidden="true">⌂</span> ' +
            SOW_t("hud.back_to_menu") +
            "</button>" +
            "    </div>" +
            "  </section>" +
            "</div>" +
            '<div class="sow-hud__map-menu hidden" id="sow-hud-map-menu" role="menu" aria-label="' +
            SOW_t("hud.map_actions") +
            '"></div>' +
            '<div class="sow-hud__placement-overlay hidden" id="sow-hud-placement-overlay">' +
            '  <button type="button" class="sow-hud__placement-arrow-btn is-up" data-placement-nudge="up" aria-label="Nudge up">▲</button>' +
            '  <button type="button" class="sow-hud__placement-arrow-btn is-right" data-placement-nudge="right" aria-label="Nudge right">▶</button>' +
            '  <button type="button" class="sow-hud__placement-arrow-btn is-down" data-placement-nudge="down" aria-label="Nudge down">▼</button>' +
            '  <button type="button" class="sow-hud__placement-arrow-btn is-left" data-placement-nudge="left" aria-label="Nudge left">◀</button>' +
            '  <div class="sow-hud__placement-panel" id="sow-hud-placement-panel" role="region" aria-label="' +
            SOW_t("hud.build_placement") +
            '"></div>' +
            "</div>" +
            '<div class="sow-hud__world-progress-overlay" id="sow-hud-world-progress-overlay" aria-hidden="true"></div>' +
            '<div class="sow-hud__world-celebrations-overlay" id="sow-hud-world-celebrations-overlay" aria-hidden="true"></div>';

        if (hudDevLayer) {
            hudDevLayer.appendChild(requiredHudQuery(hudRoot, "#sow-hud-dev-sidebar"));
        }

        hudRefs = createHudRefs();

        hudRefs.transferTarget!.appendChild(hudRefs.transferIdentity.element);
        hudRefs.betrayalTarget!.appendChild(hudRefs.betrayalIdentity.element);

        hudRefs.notifications.className = "sow-hud__notifications";
        hudRefs.notifications.setAttribute("aria-live", "polite");
        hudRoot!.appendChild(hudRefs.notifications);
        hudRefs.mapFeedback.className = "sow-hud__map-feedback";
        hudRefs.mapFeedback.setAttribute("aria-live", "polite");
        hudRefs.mapFeedback.hidden = true;
        hudRoot!.appendChild(hudRefs.mapFeedback);
        notificationCards = Array.from({ length: 3 }, function () {
            return createNotificationCard(hudRefs!.notifications, false);
        });
        mapFeedbackCard = createNotificationCard(hudRefs.mapFeedback, true);
        observeNotificationLayout();

        if (hudRefs.slider)
            hudRefs.slider.style.setProperty(
                "--sow-crossed-swords",
                'url("' + asset(HUD_ICONS.troops) + '")',
            );
        if (hudRefs.slider) {
            const allocationRail = hudRefs.leftRail;
            const refreshAllocationDetail = function () {
                const currentHud = hudState?.hud;
                const ratio = Number(hudRefs!.slider.value) / 100;
                updateArmyAllocation(currentHud?.troops, ratio, true);
            };
            hudRefs.slider.addEventListener("pointerdown", function () {
                if (allocationRail) allocationRail.classList.add("is-adjusting");
                refreshAllocationDetail();
            });
            hudRefs.slider.addEventListener("pointerup", function () {
                if (allocationRail) allocationRail.classList.remove("is-adjusting");
            });
            hudRefs.slider.addEventListener("pointercancel", function () {
                if (allocationRail) allocationRail.classList.remove("is-adjusting");
            });
            hudRefs.slider.addEventListener("keydown", function () {
                if (allocationRail) allocationRail.classList.add("is-adjusting");
            });
            hudRefs.slider.addEventListener("keyup", function () {
                if (allocationRail) allocationRail.classList.remove("is-adjusting");
            });
            hudRefs.slider.addEventListener("blur", function () {
                if (allocationRail) allocationRail.classList.remove("is-adjusting");
            });
            hudRefs.slider.addEventListener("input", function (e) {
                const input = e.currentTarget;
                if (!(input instanceof HTMLInputElement)) return;
                const val = parseFloat(input.value);
                const ratio = val / 100.0;
                const currentHud = hudState?.hud;
                updateArmyAllocation(currentHud?.troops, ratio);
                send("set_attack_ratio", { ratio: ratio });
            });
            if (allocationRail) {
                allocationRail.addEventListener("pointerenter", function (event) {
                    if (event.pointerType !== "touch") refreshAllocationDetail();
                });
                allocationRail.addEventListener("focusin", function () {
                    if (!allocationHoverNone || !allocationHoverNone.matches) refreshAllocationDetail();
                });
            }
        }
        hudDevLayer!.addEventListener("input", function (event) {
            const target = event.target;
            if (!(target instanceof Element)) return;
            const previewInput = target.closest<HTMLInputElement>("[data-dev-preview]");
            if (previewInput) {
                if (!(previewInput instanceof HTMLInputElement)) return;
                const field = previewInput.dataset.devPreview;
                if (!field || field === "target") return;
                const value =
                    previewInput.type === "checkbox" ? previewInput.checked : Number(previewInput.value);
                if (typeof value === "number" && !Number.isFinite(value)) return;
                if (field === "phase") {
                    hudDevLayer!
                        .querySelectorAll<HTMLInputElement>('[data-dev-preview="phase"]')
                        .forEach(function (sibling) {
                            if (sibling !== previewInput) sibling.value = previewInput.value;
                        });
                }
                const previewUpdate: HudCommandPayload = {};
                previewUpdate[field] = value;
                send("set_dev_vfx_preview", previewUpdate);
                return;
            }
            if (target.matches("[data-dev-search]")) {
                renderDevControls((hudState && hudState.hud && hudState.hud.dev_tools) || {});
                return;
            }
            if (target.matches("[data-dev-picker]") && target instanceof HTMLInputElement) {
                sendDevControl(target);
                return;
            }
            const input = target.closest("[data-dev]");
            if (!(input instanceof HTMLInputElement)) return;
            sendDevControl(input);
        });
        hudDevLayer!.addEventListener("pointerdown", function (event) {
            const target = event.target;
            if (!(target instanceof Element)) return;
            const button = target.closest('[data-command="dev_step"]');
            if (!(button instanceof HTMLButtonElement)) return;
            event.preventDefault();
            devStepOnce(button);
            let delay = 380;
            const repeat = function () {
                devStepTimer = window.setTimeout(function () {
                    devStepOnce(button);
                    delay = 70;
                    repeat();
                }, delay);
            };
            repeat();
        });
        ["pointerup", "pointercancel"].forEach(function (type) {
            window.addEventListener(
                type,
                function () {
                    window.clearTimeout(devStepTimer);
                },
                { passive: true },
            );
        });
        hudRoot.addEventListener("input", function (event) {
            const target = event.target;
            if (!(target instanceof Element)) return;
            const searchInput = target.closest<HTMLInputElement>("#sow-hud-lb-search");
            if (searchInput) {
                leaderboardFilterQuery = searchInput.value.trim().toLowerCase();
                applyLeaderboardFilter();
                return;
            }
            const input = target.closest<HTMLInputElement>('[data-hud-setting="music_volume"]');
            if (input) {
                const pct = Math.round(Math.max(0, Math.min(1, Number(input.value))) * 100);
                input.style.setProperty("--sow-music-fill", pct + "%");
                const musicValue = hudRefs?.settings?.querySelector<HTMLElement>("#sow-hud-music-value");
                if (musicValue) {
                    musicValue.textContent = pct + "%";
                }
            }
        });
        hudRoot.addEventListener("search", function (event) {
            const target = event.target;
            if (!(target instanceof Element)) return;
            const searchInput = target.closest<HTMLInputElement>("#sow-hud-lb-search");
            if (searchInput) {
                leaderboardFilterQuery = searchInput.value.trim().toLowerCase();
                applyLeaderboardFilter();
            }
        });
        hudRoot.addEventListener("change", function (event) {
            const target = event.target;
            if (!(target instanceof Element)) return;
            const localeInput = target.closest<HTMLInputElement>("[data-setting='locale']");
            if (localeInput) {
                if (typeof window.SOW_setLocale === "function") void window.SOW_setLocale(localeInput.value);
                return;
            }
            const input = target.closest("[data-hud-setting]");
            if (!(input instanceof HTMLInputElement)) return;
            const setting = input.dataset.hudSetting;
            if (setting === "mute_all") send("set_mute", { value: !input.checked });
            if (setting === "music_volume") {
                const pct = Math.round(Math.max(0, Math.min(1, Number(input.value))) * 100);
                input.style.setProperty("--sow-music-fill", pct + "%");
                const musicValue = hudRefs?.settings?.querySelector<HTMLElement>("#sow-hud-music-value");
                if (musicValue) {
                    musicValue.textContent = pct + "%";
                }
                send("set_music_volume", { value: Number(input.value) });
            }
            if (setting === "reduced_motion") send("set_reduced_motion", { value: input.checked });
            if (setting === "free_zoom_out") send("set_free_zoom_out", { value: input.checked });
            if (setting === "show_fps_ping") send("set_show_fps_ping", { value: input.checked });
            if (setting === "show_dev_tools") {
                if (!input.checked) {
                    devSidebarOpen = false;
                }
                send("set_show_dev_tools", { value: input.checked });
            }
        });
    }

    const mapActionLabels: Record<string, { icon?: string; key: string; status?: string }> = {
        emote: { icon: "emote", key: "hud.map_action_emote" },
        spawn: { icon: "spawn", key: "hud.map_action_deploy" },
        attack: { icon: "attack", key: "hud.map_action_attack" },
        fleet: { icon: "fleet", key: "hud.map_action_fleet" },
        transfer: { icon: "transfer", key: "hud.map_action_transfer" },
        alliance: { status: "allied", key: "hud.map_action_alliance" },
        build_city: { key: "hud.map_action_city" },
        build_factory: { key: "hud.map_action_factory" },
        build_port: { key: "hud.map_action_port" },
        build_tower: { key: "hud.map_action_tower" },
        build_farm: { key: "hud.map_action_farm" },
        upgrade_structure: { icon: "upgrade", key: "hud.map_action_upgrade" },
        nuke: { icon: "nuke", key: "hud.map_action_nuke" },
    };

    const allianceActionLabels: Record<
        string,
        { state: string; key: string; status: string; disabled: boolean }
    > = {
        request: {
            state: "request",
            key: "hud.map_action_request_alliance",
            status: "allied",
            disabled: false,
        },
        accept: { state: "accept", key: "hud.map_action_accept_alliance", status: "allied", disabled: false },
        pending: {
            state: "pending",
            key: "hud.map_action_alliance_pending",
            status: "request",
            disabled: true,
        },
        active: { state: "active", key: "hud.map_action_alliance_active", status: "allied", disabled: true },
        renew: { state: "renew", key: "hud.map_action_renew_alliance", status: "allied", disabled: false },
    };

    function mapLabel(action: string): string {
        const label = mapActionLabels[action];
        return label ? SOW_t(label.key) : action;
    }

    function updateArmyAllocation(
        totalTroops: number | undefined,
        ratio: number,
        forceDetails = false,
    ): void {
        if (!hudRefs) return;
        const troopCount = Math.max(0, Number(totalTroops) || 0);
        const total = Math.floor(troopCount);
        let share = Number(ratio);
        if (!Number.isFinite(share)) share = 0.5;
        share = Math.max(0, Math.min(1, share));
        const percent = Math.round(share * 100);
        if (hudRefs.armyRatio && hudRefs.armyRatio.textContent !== percent + "%")
            hudRefs.armyRatio.textContent = percent + "%";
        const fill = ((percent - 5) / 95) * 100 + "%";
        if (hudRefs.slider && hudRefs.slider.style.getPropertyValue("--sow-ratio") !== fill)
            hudRefs.slider.style.setProperty("--sow-ratio", fill);
        const allocationRail = hudRefs.leftRail;
        const detailsVisible =
            forceDetails ||
            (allocationRail &&
                (allocationRail.classList.contains("is-adjusting") ||
                    (!(allocationHoverNone && allocationHoverNone.matches) &&
                        (allocationRail.matches(":hover") ||
                            allocationRail.contains(document.activeElement)))));
        if (!detailsVisible) return;
        const allocationKey = total + ":" + share + ":" + String(window.SOW_LOCALE || "en");
        if (allocationKey === lastAllocationKey) return;
        lastAllocationKey = allocationKey;
        const sent = Math.min(total, Math.floor(troopCount * share));
        const kept = total - sent;
        const text = SOW_t("hud.attack_allocation", {
            send: sent.toLocaleString(),
            keep: kept.toLocaleString(),
            ratio: Math.round(share * 100),
        });
        if (hudRefs.armyAllocation && hudRefs.armyAllocation.textContent !== text) {
            hudRefs.armyAllocation.textContent = text;
        }
        if (hudRefs.slider && hudRefs.slider.getAttribute("aria-valuetext") !== text) {
            hudRefs.slider.setAttribute("aria-valuetext", text);
        }
    }

    function mapActionReason(item: HudActionReasonItem, availableGold: number | undefined): string {
        if (item.reason_key) {
            let reason = SOW_t(
                item.reason_key,
                item.reason_troops == null
                    ? {}
                    : { troops: Math.ceil(Number(item.reason_troops)).toLocaleString() },
            );
            if (item.reason_level != null || item.reason_seconds != null) {
                if (item.reason_key !== "lobbies.locked") reason = SOW_t("lobbies.locked") + " · " + reason;
                if (item.reason_level != null) reason += " " + Math.floor(Number(item.reason_level));
                if (item.reason_seconds != null)
                    reason += " · " + Math.max(1, Math.ceil(Number(item.reason_seconds))) + "s";
            }
            return reason;
        }
        if (!item.disabled || item.cost == null) return "";
        return SOW_t("hud.not_enough_gold", {
            cost: Math.ceil(item.cost).toLocaleString(),
            available: Math.floor(Number(availableGold) || 0).toLocaleString(),
        });
    }

    function campaignUnlocks(
        hud: MenuHudPayload | null | undefined,
    ): { buildings: Record<string, number>; actions: string[] } | null {
        return hud && hud.tutorial && hud.tutorial.active
            ? hud.tutorial.effective_unlocks || EMPTY_CAMPAIGN_UNLOCKS
            : null;
    }
    function campaignAllowsBuilding(
        unlocks: { buildings: Record<string, number>; actions: string[] } | null,
        kind: string,
        level: number,
    ): boolean {
        return unlocks === null || window.SOWCampaign.campaignAllowsBuilding(unlocks, kind, level);
    }
    function campaignAllowsAction(
        unlocks: { buildings: Record<string, number>; actions: string[] } | null,
        action: string,
    ): boolean {
        return unlocks === null || window.SOWCampaign.campaignAllowsAction(unlocks, action);
    }
    function setCampaignLock(element: HTMLElement, locked: boolean): void {
        element.classList.toggle("is-campaign-locked", Boolean(locked));
    }

    function mapItems(mapMenu: HudMapMenu): HudMapMenuItem[] {
        const actions = Array.isArray(mapMenu.actions) ? mapMenu.actions : [];
        const supplied = Array.isArray(mapMenu.items) ? mapMenu.items : [];
        return actions.map(function (action) {
            const item = supplied.find(function (candidate) {
                return candidate && candidate.action === action;
            });
            return {
                action: action,
                cost:
                    item && item.cost != null && Number.isFinite(Number(item.cost))
                        ? Number(item.cost)
                        : null,
                level: item && Number.isFinite(Number(item.level)) ? Number(item.level) : null,
                disabled: Boolean(item && item.disabled),
                reason_key: (item && item.reason_key) || null,
                reason_level:
                    item && Number.isFinite(Number(item.reason_level)) ? Number(item.reason_level) : null,
                reason_seconds:
                    item && Number.isFinite(Number(item.reason_seconds)) ? Number(item.reason_seconds) : null,
                reason_troops:
                    item && Number.isFinite(Number(item.reason_troops)) ? Number(item.reason_troops) : null,
            };
        });
    }

    function patchBuildingDetailsCard(
        menu: HTMLElement,
        building: HudMapMenuBuilding,
        availableGold: number | undefined,
    ): void {
        if (building.under_construction || building.remaining_seconds != null) {
            patchBuildingProgressBar(menu, building.remaining_seconds, building.progress);
        }
        const upgrade = building.upgrade;
        if (!upgrade) return;
        const costVal = Math.max(0, Math.ceil(Number(upgrade.cost) || 0));
        const costText = formatHudMetric(costVal);
        const timeText = Math.max(1, Math.ceil(Number(upgrade.seconds) || 0)) + "s";
        const enabled = Boolean(upgrade.enabled);
        const costEl = menu.querySelector<HTMLElement>("[data-building-cost]");
        if (costEl && costEl.textContent !== costText) {
            costEl.textContent = costText;
        }
        const timeEl = menu.querySelector<HTMLElement>("[data-building-time]");
        if (timeEl && timeEl.textContent !== timeText) {
            timeEl.textContent = timeText;
        }
        const button = menu.querySelector<HTMLButtonElement>(".sow-hud__building-upgrade-btn");
        if (button) {
            button.disabled = false;
            button.classList.toggle("is-locked", !enabled);
            button.setAttribute("aria-disabled", String(!enabled));
            const baseLabel = SOW_t("hud.map_action_upgrade") + " Lv." + upgrade.next_level;
            const reason = !enabled
                ? mapActionReason({ disabled: true, cost: upgrade.cost }, availableGold)
                : costText + "g · " + timeText;
            const accessibleLabel = reason ? baseLabel + ". " + reason : baseLabel;
            if (button.getAttribute("aria-label") !== accessibleLabel) {
                button.setAttribute("aria-label", accessibleLabel);
            }
        }
    }

    function renderBuildingProgressBarMarkup(kind: string, level: number): string {
        return (
            '<div class="sow-hud__building-progress-row" role="status">' +
            '<span class="sow-hud__building-card-thumb" aria-hidden="true">' +
            buildingIcon(kind, Math.max(1, level), "sow-hud__building-card-art") +
            "</span>" +
            '<div class="sow-hud__building-progress-track" role="progressbar" aria-valuemin="0" aria-valuemax="100">' +
            '<div class="sow-hud__building-progress-fill" data-building-progress-fill></div>' +
            '<b class="sow-hud__building-progress-time" data-building-timer></b>' +
            "</div>" +
            "</div>"
        );
    }

    function patchBuildingProgressBar(
        container: ParentNode,
        remainingSeconds: number | null | undefined,
        progress: number | null | undefined,
    ): void {
        const timerEl = container.querySelector<HTMLElement>("[data-building-timer]");
        const secsText = Math.max(1, Math.ceil(Number(remainingSeconds) || 0)) + "s";
        if (timerEl && timerEl.textContent !== secsText) {
            timerEl.textContent = secsText;
        }
        const fillEl = container.querySelector<HTMLElement>("[data-building-progress-fill]");
        if (fillEl) {
            const pct = Math.max(0, Math.min(100, (Number(progress) || 0) * 100));
            const widthText = pct.toFixed(1) + "%";
            if (fillEl.style.width !== widthText) {
                fillEl.style.width = widthText;
            }
        }
    }

    function positionBuildingDetailsCard(menu: HTMLElement, mapMenu: HudMapMenu): void {
        const bld = mapMenu.building;
        const anchorX = Number.isFinite(Number(bld && bld.anchor_x))
            ? Number(bld!.anchor_x)
            : Number(mapMenu.x || 0);
        const anchorY = Number.isFinite(Number(bld && bld.anchor_y))
            ? Number(bld!.anchor_y)
            : Number(mapMenu.y || 0);
        const anchorRadius = Math.max(20, Number(bld && bld.anchor_radius) || 28);
        const card = menu.querySelector<HTMLElement>(".sow-hud__building-card") || menu;
        const width = card.offsetWidth || 232;
        const height = card.offsetHeight || 114;
        const halfW = width * 0.5;
        const halfH = height * 0.5;
        const vw = window.innerWidth || 800;
        const vh = window.innerHeight || 600;
        let bottomLimit = vh - 96;
        if (
            hudRefs &&
            hudRefs.dock &&
            !hudRefs.dock.hidden &&
            typeof hudRefs.dock.getBoundingClientRect === "function"
        ) {
            const dockRect = hudRefs.dock.getBoundingClientRect();
            if (dockRect && dockRect.height > 0 && dockRect.top > 0) {
                bottomLimit = Math.min(bottomLimit, dockRect.top - 10);
            }
        }
        const gap = 10;
        const belowCenterY = anchorY + anchorRadius + gap + halfH;
        const aboveCenterY = anchorY - anchorRadius - gap - halfH;
        const placeBelow = belowCenterY + halfH <= bottomLimit || aboveCenterY - halfH < 12;
        const rawCenterY = placeBelow ? belowCenterY : aboveCenterY;
        const minX = halfW + 10;
        const maxX = Math.max(minX, vw - halfW - 10);
        const minY = halfH + 10;
        const maxY = Math.max(minY, Math.min(vh - halfH - 10, bottomLimit - halfH));
        const left = Math.min(Math.max(anchorX, minX), maxX);
        const top = Math.min(Math.max(rawCenterY, minY), maxY);
        menu.style.left = left.toFixed(1) + "px";
        menu.style.top = top.toFixed(1) + "px";
        menu.dataset.placement = placeBelow ? "below" : "above";
        const caretLimit = Math.max(0, halfW - 18);
        const caretOffsetX = Math.max(-caretLimit, Math.min(caretLimit, anchorX - left));
        menu.style.setProperty("--caret-offset-x", caretOffsetX.toFixed(1) + "px");
    }

    function renderBuildingDetailsCard(menu: HTMLElement, mapMenu: HudMapMenu, hud: HudRingState): boolean {
        const building = mapMenu.building;
        if (!building || !building.kind) {
            menu.classList.add("hidden");
            menu.classList.remove("is-building-card", "has-tether");
            if (menu.dataset.renderKey) menu.replaceChildren();
            menu.dataset.renderKey = "";
            delete menu.dataset.placement;
            delete menu.dataset.level;
            return false;
        }
        const kind = String(building.kind);
        const level = Math.max(0, Math.floor(Number(building.level) || 0));
        const maxLevel = Math.max(level, Math.floor(Number(building.max_level) || level));
        const underConstruction = Boolean(building.under_construction) || building.remaining_seconds != null;
        const chips = Array.isArray(building.chips) ? building.chips : [];
        const upgrade = !underConstruction && building.upgrade ? building.upgrade : null;
        const session = String(mapMenu.session) + ":" + String(mapMenu.tile_idx);
        menu.dataset.session = String(mapMenu.session);
        menu.dataset.tileIdx = String(mapMenu.tile_idx);
        delete menu.dataset.level;
        menu.classList.remove("hidden", "has-tether");
        menu.classList.add("is-building-card");

        const renderKey = [
            window.SOW_LOCALE || "en",
            "building_details",
            session,
            kind,
            level,
            maxLevel,
            underConstruction ? "constructing" : upgrade ? "upgrade:" + upgrade.next_level : "max",
            chips
                .map(function (chip) {
                    return chip.icon + "=" + chip.label;
                })
                .join("|"),
        ].join(":");

        if (menu.dataset.renderKey !== renderKey) {
            const card = document.createElement("div");
            card.dataset.buildingKind = kind.toLowerCase();
            if (underConstruction) {
                const targetLv = level > 0 ? level + 1 : 1;
                card.className = "sow-hud__building-card is-constructing";
                card.innerHTML =
                    '<span class="sow-hud__building-card-caret" aria-hidden="true"></span>' +
                    renderBuildingProgressBarMarkup(kind, targetLv);
            } else {
                const buildingTitle = SOW_t("hud.map_action_" + kind.toLowerCase());
                const isMax = !upgrade && maxLevel > 0 && level >= maxLevel;
                const levelBadgeHtml = upgrade
                    ? "Lv." +
                      level +
                      ' <span class="sow-hud__building-card-level-arrow">→</span> <em>Lv.' +
                      upgrade.next_level +
                      "</em>"
                    : "Lv." + Math.max(1, level) + (isMax ? " · MAX" : "");

                const chipsHtml = chips.length
                    ? '<div class="sow-hud__building-card-stats">' +
                      chips
                          .map(function (chip) {
                              const label = chip.label
                                  ? '<b class="sow-hud__building-stat-value">' +
                                    escapeHudText(chip.label) +
                                    "</b>"
                                  : "";
                              return (
                                  '<span class="sow-hud__building-stat-pill' +
                                  (upgrade ? " is-gain" : "") +
                                  '" data-stat-icon="' +
                                  escapeHudText(chip.icon) +
                                  '">' +
                                  statusIcon(chip.icon, "sow-hud__building-stat-icon") +
                                  label +
                                  "</span>"
                              );
                          })
                          .join("") +
                      "</div>"
                    : "";

                const footerHtml = upgrade
                    ? '<button type="button" class="sow-hud__building-upgrade-btn" data-map-action="upgrade_structure">' +
                      '<span class="sow-hud__building-upgrade-cta">' +
                      statusIcon("arrow-up", "sow-hud__building-upgrade-arrow") +
                      '<span class="sow-hud__building-upgrade-label">' +
                      escapeHudText(SOW_t("hud.map_action_upgrade")) +
                      " <em>Lv." +
                      upgrade.next_level +
                      "</em></span>" +
                      "</span>" +
                      '<span class="sow-hud__building-upgrade-meta">' +
                      '<span class="sow-hud__building-upgrade-pill sow-hud__building-upgrade-cost">' +
                      hudIcon("gold", "sow-hud__building-upgrade-meta-icon") +
                      "<b data-building-cost></b>" +
                      "</span>" +
                      '<span class="sow-hud__building-upgrade-pill sow-hud__building-upgrade-time">' +
                      statusIcon("clock", "sow-hud__building-upgrade-meta-icon") +
                      "<b data-building-time></b>" +
                      "</span>" +
                      "</span>" +
                      "</button>"
                    : "";

                card.className = "sow-hud__building-card";
                card.innerHTML =
                    '<span class="sow-hud__building-card-caret" aria-hidden="true"></span>' +
                    '<div class="sow-hud__building-card-head">' +
                    '<span class="sow-hud__building-card-thumb" aria-hidden="true">' +
                    buildingIcon(kind, Math.max(1, level), "sow-hud__building-card-art") +
                    "</span>" +
                    '<div class="sow-hud__building-card-identity">' +
                    '<strong class="sow-hud__building-card-title">' +
                    escapeHudText(buildingTitle) +
                    "</strong>" +
                    '<span class="sow-hud__building-card-level' +
                    (isMax ? " is-max" : upgrade ? " is-upgradable" : "") +
                    '">' +
                    levelBadgeHtml +
                    "</span>" +
                    "</div>" +
                    "</div>" +
                    chipsHtml +
                    footerHtml;
            }
            menu.replaceChildren(card);
            menu.dataset.renderKey = renderKey;
        }

        patchBuildingDetailsCard(menu, building, hud.gold);
        positionBuildingDetailsCard(menu, mapMenu);
        return true;
    }

    function positionPlacementOverlay(overlay: HTMLElement, placement: HudBuildingPlacement): void {
        const vw = window.innerWidth || 800;
        const vh = window.innerHeight || 600;
        const anchorX = Number.isFinite(Number(placement.anchor_x)) ? Number(placement.anchor_x) : vw * 0.5;
        const anchorY = Number.isFinite(Number(placement.anchor_y)) ? Number(placement.anchor_y) : vh * 0.5;

        // Position 4 arrows around the building footprint
        const arrows = placement.arrows;
        const refs = hudRefs && hudRefs.placementArrows;
        if (arrows && refs) {
            refs.up.style.left = arrows.up[0].toFixed(1) + "px";
            refs.up.style.top = arrows.up[1].toFixed(1) + "px";
            refs.right.style.left = arrows.right[0].toFixed(1) + "px";
            refs.right.style.top = arrows.right[1].toFixed(1) + "px";
            refs.down.style.left = arrows.down[0].toFixed(1) + "px";
            refs.down.style.top = arrows.down[1].toFixed(1) + "px";
            refs.left.style.left = arrows.left[0].toFixed(1) + "px";
            refs.left.style.top = arrows.left[1].toFixed(1) + "px";
        }

        // Position the action card directly below the down arrow
        const panel = hudRefs && hudRefs.placementPanel;
        if (!panel) return;
        const width = panel.offsetWidth || 230;
        const height = panel.offsetHeight || 96;
        const halfW = width * 0.5;
        const halfH = height * 0.5;

        let bottomLimit = vh - 96;
        if (
            hudRefs &&
            hudRefs.dock &&
            !hudRefs.dock.hidden &&
            typeof hudRefs.dock.getBoundingClientRect === "function"
        ) {
            const dockRect = hudRefs.dock.getBoundingClientRect();
            if (dockRect && dockRect.height > 0 && dockRect.top > 0) {
                bottomLimit = Math.min(bottomLimit, dockRect.top - 10);
            }
        }

        const downArrowY = arrows ? arrows.down[1] : anchorY + 48;
        const upArrowY = arrows ? arrows.up[1] : anchorY - 48;
        const gap = 14;
        const belowCenterY = downArrowY + gap + halfH;
        const aboveCenterY = upArrowY - gap - halfH;
        const placeBelow = belowCenterY + halfH <= bottomLimit || aboveCenterY - halfH < 64;
        const rawCenterY = placeBelow ? belowCenterY : aboveCenterY;
        const minX = halfW + 10;
        const maxX = Math.max(minX, vw - halfW - 10);
        const minY = halfH + 10;
        const maxY = Math.max(minY, Math.min(vh - halfH - 10, bottomLimit - halfH));
        const left = Math.min(Math.max(anchorX, minX), maxX);
        const top = Math.min(Math.max(rawCenterY, minY), maxY);
        panel.style.left = left.toFixed(1) + "px";
        panel.style.top = top.toFixed(1) + "px";
        panel.dataset.placement = placeBelow ? "below" : "above";
    }

    function renderPlacementPanel(placement: HudBuildingPlacement | null | undefined): boolean {
        const overlay = hudRefs && hudRefs.placementOverlay;
        const panel = hudRefs && hudRefs.placementPanel;
        if (!overlay || !panel) return false;
        if (!placement || placement.pending) {
            overlay.classList.add("hidden");
            if (panel.dataset.renderKey) panel.replaceChildren();
            panel.dataset.renderKey = "";
            delete panel.dataset.placement;
            return false;
        }

        const kind = String(placement.kind || "City");
        const tileIdx = placement.tile_idx != null ? String(placement.tile_idx) : "0";
        const canConfirm = Boolean(placement.can_confirm);
        const cost = Number(placement.cost || 0);
        const reasonKey = placement.reason || "";
        const locale = window.SOW_LOCALE || "en";
        const renderKey = [locale, kind, tileIdx, canConfirm ? "1" : "0", String(cost), reasonKey].join(":");

        overlay.classList.remove("hidden");

        if (panel.dataset.renderKey !== renderKey) {
            const buildingTitle = SOW_t("hud.map_action_" + kind.toLowerCase());
            const reason = reasonKey ? SOW_t(reasonKey) : null;
            const cancelLabel = SOW_t("lobbies.cancel");
            const buildLabel = SOW_t("hud.build_confirm");

            panel.innerHTML =
                '<div class="sow-hud__placement-panel-head">' +
                '<span class="sow-hud__placement-panel-thumb" aria-hidden="true">' +
                buildingIcon(kind, 1, "sow-hud__placement-panel-art") +
                "</span>" +
                '<div class="sow-hud__placement-panel-identity">' +
                '<strong class="sow-hud__placement-panel-title">' +
                escapeHudText(buildingTitle) +
                "</strong>" +
                (reason
                    ? '<span class="sow-hud__placement-panel-reason">' + escapeHudText(reason) + "</span>"
                    : '<span class="sow-hud__placement-panel-cost">' +
                      hudIcon("gold", "sow-hud__inline-icon") +
                      " <b>" +
                      formatEndgameExact(cost) +
                      "</b></span>") +
                "</div>" +
                "</div>" +
                '<div class="sow-hud__placement-panel-controls">' +
                '<button type="button" class="sow-hud__placement-action-btn is-cancel" data-placement-action="cancel" aria-label="' +
                escapeHudText(cancelLabel) +
                '">' +
                '<span class="sow-hud__placement-btn-icon" aria-hidden="true">✕</span>' +
                "<span>" +
                escapeHudText(cancelLabel) +
                "</span>" +
                "</button>" +
                '<button type="button" class="sow-hud__placement-action-btn is-confirm' +
                (canConfirm ? "" : " is-disabled") +
                '" data-placement-action="confirm"' +
                (canConfirm ? "" : " disabled") +
                ' aria-label="' +
                escapeHudText(buildLabel) +
                '">' +
                '<span class="sow-hud__placement-btn-icon" aria-hidden="true">✓</span>' +
                "<span>" +
                escapeHudText(buildLabel) +
                "</span>" +
                "</button>" +
                "</div>";
            panel.dataset.renderKey = renderKey;
        }

        positionPlacementOverlay(overlay, placement);
        return true;
    }

    const activeProgressBars: Map<number, HTMLElement> = new Map();
    const activeCelebrations: Set<string> = new Set();

    function renderWorldBuildingProgress(items: HudBuildingProgressItem[] | null | undefined): void {
        const overlay = hudRefs && hudRefs.worldProgressOverlay;
        if (!overlay) return;

        if (!items || items.length === 0) {
            if (activeProgressBars.size > 0) {
                overlay.replaceChildren();
                activeProgressBars.clear();
            }
            return;
        }

        const currentTiles = new Set<number>();
        for (let i = 0; i < items.length; i++) {
            const item = items[i];
            currentTiles.add(item.tile_idx);
            let bar = activeProgressBars.get(item.tile_idx);
            if (!bar) {
                bar = document.createElement("div");
                bar.className = "sow-hud__world-building-bar";
                bar.dataset.tile = String(item.tile_idx);
                bar.innerHTML =
                    '<span class="sow-hud__world-building-thumb" aria-hidden="true">' +
                    buildingIcon(item.kind, Math.max(1, item.level), "sow-hud__world-building-art") +
                    "</span>" +
                    '<div class="sow-hud__world-building-track" role="progressbar">' +
                    '<div class="sow-hud__world-building-fill" data-building-progress-fill></div>' +
                    '<b class="sow-hud__world-building-time" data-building-timer></b>' +
                    "</div>";
                overlay.appendChild(bar);
                activeProgressBars.set(item.tile_idx, bar);
            }

            bar.style.display = item.visible === false ? "none" : "flex";
            bar.style.left = item.screen_x.toFixed(1) + "px";
            bar.style.top = item.screen_y.toFixed(1) + "px";

            patchBuildingProgressBar(bar, item.remaining_seconds, item.progress);
        }

        for (const [tile, el] of activeProgressBars.entries()) {
            if (!currentTiles.has(tile)) {
                el.remove();
                activeProgressBars.delete(tile);
            }
        }
    }

    function renderWorldBuildingCelebrations(items: HudBuildingCelebrationItem[] | null | undefined): void {
        const overlay = hudRefs && hudRefs.worldCelebrationsOverlay;
        if (!overlay || !items || items.length === 0) return;

        for (let i = 0; i < items.length; i++) {
            const item = items[i];
            const celebrationKey = item.tile_idx + ":" + item.level;
            if (activeCelebrations.has(celebrationKey) || item.visible === false) {
                continue;
            }
            activeCelebrations.add(celebrationKey);

            const container = document.createElement("div");
            container.className = "sow-hud__world-celebration";
            container.style.left = item.screen_x.toFixed(1) + "px";
            container.style.top = item.screen_y.toFixed(1) + "px";

            let sparklesHtml = "";
            const offsets = [
                [-30, -32, 0],
                [30, -30, 40],
                [-18, -48, 80],
                [18, -46, 60],
                [-38, -14, 100],
                [38, -16, 90],
            ];
            for (let j = 0; j < offsets.length; j++) {
                const [dx, dy, delay] = offsets[j];
                sparklesHtml +=
                    '<span class="sow-hud__celebration-sparkle" style="--dx:' +
                    dx +
                    "px;--dy:" +
                    dy +
                    "px;--delay:" +
                    delay +
                    'ms;"></span>';
            }

            container.innerHTML =
                '<span class="sow-hud__celebration-ring"></span>' +
                sparklesHtml +
                '<div class="sow-hud__celebration-badge">' +
                '<span class="sow-hud__celebration-badge-thumb" aria-hidden="true">' +
                buildingIcon(item.kind, Math.max(1, item.level), "sow-hud__celebration-badge-art") +
                "</span>" +
                '<b class="sow-hud__celebration-badge-text">LV ' +
                item.level +
                "</b>" +
                "</div>";

            overlay.appendChild(container);

            setTimeout(function () {
                container.remove();
                activeCelebrations.delete(celebrationKey);
            }, 1200);
        }
    }

    // Command ring: six fixed 60° slots, clockwise from the top. An action keeps
    // its slot in every context, so the ring never changes shape; actions that do
    // not apply leave an empty notch. Submenus reuse the same six slots.
    const RING_SLOTS = 6;
    const RING_HOLD_MS = 600;
    const RING_HUB_RADIUS = 19;
    const RING_INNER_RADIUS = 21;
    const RING_OUTER_RADIUS = 48;
    const RING_ICON_RADIUS = 34.5;
    const RING_ROOT_SLOTS: Record<string, number> = {
        attack: 0,
        spawn: 0,
        upgrade_structure: 0,
        build: 0,
        fleet: 1,
        nuke: 2,
        alliance: 3,
        transfer: 4,
        emote: 5,
    };
    const RING_BUILD_SLOTS: Record<string, number> = {
        build_city: 0,
        build_factory: 1,
        build_port: 2,
        build_tower: 3,
        build_farm: 4,
    };
    const RING_EMOTE_SLOTS: Record<string, number> = { greet: 0, celebrate: 1, threat: 2, help: 3 };
    const RING_ACCENTS: Record<string, string> = {
        attack: "red",
        nuke: "red",
        spawn: "green",
        alliance: "green",
        transfer: "green",
        fleet: "cyan",
    };
    const RING_HOLD_ACTIONS: Record<string, boolean> = { nuke: true };
    const EXPRESSION_CELLS: string[] = ["happy", "calm", "fear", "angry"];

    function ringSlots(mapMenu: HudMapMenu, items: HudMapMenuItem[], hud: HudRingState): (RingSlot | null)[] {
        const slots: (RingSlot | null)[] = new Array(RING_SLOTS).fill(null);
        if (mapMenuView === "emote") {
            (hud.emotes || []).forEach(function (definition) {
                const index = RING_EMOTE_SLOTS[definition.kind];
                if (index != null) slots[index] = { type: "emote", definition: definition };
            });
            if (hud.emote_active) slots[4] = { type: "clear" };
            slots[5] = { type: "pin", enabled: Boolean(hud.emote_active) };
            return slots;
        }
        if (mapMenuView === "build") {
            items.forEach(function (item) {
                const index = RING_BUILD_SLOTS[item.action];
                if (index != null) slots[index] = { type: "action", item: item, presentation: null };
            });
            return slots;
        }
        items.forEach(function (item) {
            if (RING_BUILD_SLOTS[item.action] != null) slots[0] = { type: "group", group: "build" };
            else if (item.action === "emote") slots[5] = { type: "group", group: "emote" };
            else if (RING_ROOT_SLOTS[item.action] != null) {
                slots[RING_ROOT_SLOTS[item.action]] = {
                    type: "action",
                    item: item,
                    presentation:
                        item.action === "alliance" && mapMenu.alliance_state
                            ? allianceActionLabels[mapMenu.alliance_state]
                            : null,
                };
            }
        });
        return slots;
    }

    function ringSlotIdentity(slot: RingSlot | null): string {
        if (!slot) return "-";
        if (slot.type === "action") return slot.item.action;
        if (slot.type === "group") return "group:" + slot.group;
        if (slot.type === "emote") return "emote:" + slot.definition.kind;
        return slot.type;
    }

    function ringSlotAccent(slot: RingSlot | null): string {
        if (slot && slot.type === "action") return RING_ACCENTS[slot.item.action] || "gold";
        return "gold";
    }

    function ringSlotLabel(slot: RingSlot): string {
        if (slot.type === "action")
            return slot.presentation ? SOW_t(slot.presentation.key) : mapLabel(slot.item.action);
        if (slot.type === "group")
            return SOW_t(slot.group === "emote" ? "hud.map_action_emote" : "hud.map_action_build");
        if (slot.type === "emote") return SOW_t(slot.definition.menu_key);
        return SOW_t(slot.type === "clear" ? "hud.emote_clear" : "hud.emote_pin");
    }

    function ringSlotDisabled(slot: RingSlot): boolean {
        if (slot.type === "pin") return !slot.enabled;
        return Boolean(
            slot.type === "action" &&
            (slot.item.disabled || (slot.presentation && slot.presentation.disabled)),
        );
    }

    function ringSlotDetail(slot: RingSlot, availableGold: number | undefined): string {
        if (slot.type !== "action") return "";
        const reason = mapActionReason(slot.item, availableGold);
        if (reason) return escapeHudText(reason);
        const parts = [];
        if (slot.item.level != null) parts.push("Lv " + slot.item.level + " → " + (slot.item.level + 1));
        if (slot.item.cost != null)
            parts.push(
                hudIcon("gold", "sow-hud__ring-readout-icon") +
                    escapeHudText(Math.ceil(slot.item.cost).toLocaleString()),
            );
        return parts.join(" · ");
    }

    function ringPoint(angle: number, radius: number): string {
        const radians = (angle * Math.PI) / 180;
        return (
            (50 + Math.sin(radians) * radius).toFixed(2) + " " + (50 - Math.cos(radians) * radius).toFixed(2)
        );
    }

    function ringSectorMarkup(slot: RingSlot | null, index: number): string {
        const span = 360 / RING_SLOTS;
        const start = index * span - span / 2 + 1.6;
        const end = index * span + span / 2 - 1.6;
        const wedge =
            "M" +
            ringPoint(start, RING_OUTER_RADIUS) +
            " A" +
            RING_OUTER_RADIUS +
            " " +
            RING_OUTER_RADIUS +
            " 0 0 1 " +
            ringPoint(end, RING_OUTER_RADIUS) +
            " L" +
            ringPoint(end, RING_INNER_RADIUS) +
            " A" +
            RING_INNER_RADIUS +
            " " +
            RING_INNER_RADIUS +
            " 0 0 0 " +
            ringPoint(start, RING_INNER_RADIUS) +
            " Z";
        const rimRadius = RING_OUTER_RADIUS - 1.6;
        const rim =
            "M" +
            ringPoint(start + 1, rimRadius) +
            " A" +
            rimRadius +
            " " +
            rimRadius +
            " 0 0 1 " +
            ringPoint(end - 1, rimRadius);
        return (
            '<g class="sow-hud__ring-sector' +
            (slot ? "" : " is-empty") +
            '" data-slot="' +
            index +
            '" data-accent="' +
            ringSlotAccent(slot) +
            '">' +
            '<path class="sow-hud__ring-wedge" d="' +
            wedge +
            '"/>' +
            (slot
                ? '<path class="sow-hud__ring-rim" d="' +
                  rim +
                  '" pathLength="1"/><path class="sow-hud__ring-hold" d="' +
                  rim +
                  '" pathLength="1"/>'
                : "") +
            "</g>"
        );
    }

    function ringFace(expression: string): HTMLElement {
        const hud = hudState?.hud;
        const leader = leaderById(hud?.player_leader || hudState?.selected_leader);
        const cell = EXPRESSION_CELLS.indexOf(expression);
        const face = document.createElement("span");
        face.className = "sow-hud__ring-face";
        face.innerHTML = hudIcon("emote", "sow-hud__ring-icon");
        if (cell < 0 || !leader || !(leader.art_slug || leader.slug)) return face;
        face.style.setProperty("--face-x", String(cell % 2));
        face.style.setProperty("--face-y", String(Math.floor(cell / 2)));
        const image = document.createElement("img");
        image.alt = "";
        image.onerror = function () {
            this.remove();
        };
        image.src = asset("gameplay/avatars/spritesheets/" + (leader.art_slug || leader.slug) + ".webp");
        face.appendChild(image);
        return face;
    }

    function ringSlotIcon(slot: RingSlot, mapMenu: HudMapMenu): HTMLElement {
        if (slot.type === "emote") return ringFace(slot.definition.expression);
        if (slot.type === "clear") return ringFace("calm");
        const title = document.createElement("span");
        if (slot.type === "pin") title.innerHTML = hudIcon("anchor", "sow-hud__ring-icon");
        else if (slot.type === "group")
            title.innerHTML = hudIcon(
                slot.group === "emote" ? "emote" : "construction",
                "sow-hud__ring-icon",
            );
        else {
            const action = slot.item.action;
            const label = mapActionLabels[action] || {};
            const status = (slot.presentation && slot.presentation.status) || label.status;
            let building =
                RING_BUILD_SLOTS[action] != null
                    ? buildingIcon(action.slice(6), 1, "sow-hud__building-icon sow-hud__ring-icon")
                    : "";
            if (action === "upgrade_structure" && mapMenu.building)
                building = buildingIcon(
                    mapMenu.building.kind,
                    mapMenu.building.level,
                    "sow-hud__building-icon sow-hud__ring-icon",
                );
            title.innerHTML = status
                ? statusIcon(status, "sow-hud__ring-icon")
                : building || hudIcon(label.icon || "plus", "sow-hud__ring-icon");
        }
        return title;
    }

    function ringSlotButton(
        slot: RingSlot,
        index: number,
        mapMenu: HudMapMenu,
        hud: HudRingState,
    ): HTMLButtonElement {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "sow-hud__ring-slot";
        button.dataset.slot = String(index);
        button.dataset.accent = ringSlotAccent(slot);
        button.setAttribute("role", "menuitem");
        const angle = (((index * 360) / RING_SLOTS) * Math.PI) / 180;
        button.style.left = (50 + Math.sin(angle) * RING_ICON_RADIUS).toFixed(2) + "%";
        button.style.top = (50 - Math.cos(angle) * RING_ICON_RADIUS).toFixed(2) + "%";
        button.style.setProperty("--slot-angle", (index * 360) / RING_SLOTS + "deg");
        button.style.setProperty("--slot-index", String(index));
        if (slot.type === "action") {
            button.dataset.mapAction = slot.item.action;
            if (RING_HOLD_ACTIONS[slot.item.action]) button.dataset.ringHold = "true";
            if (slot.presentation && slot.presentation.state)
                button.dataset.allianceState = slot.presentation.state;
        } else if (slot.type === "group") button.dataset.mapGroup = slot.group;
        else if (slot.type === "emote") button.dataset.emoteKind = slot.definition.kind;
        else if (slot.type === "clear") button.dataset.command = "clear_emote";
        else {
            button.dataset.emotePinToggle = "true";
            button.setAttribute("aria-pressed", String(Boolean(hud.emote_pinned)));
        }
        const icon = ringSlotIcon(slot, mapMenu);
        icon.classList.add("sow-hud__map-action-title");
        button.appendChild(icon);
        const badge = document.createElement("span");
        badge.className = "sow-hud__ring-badge";
        badge.setAttribute("aria-hidden", "true");
        button.appendChild(badge);
        return button;
    }

    function ringHub(mapMenu: HudMapMenu): HTMLElement {
        let hub;
        if (mapMenuView === "root") {
            hub = document.createElement("span");
            if (mapMenu.target_avatar) {
                const image = document.createElement("img");
                image.className = "sow-hud__ring-portrait";
                image.alt = "";
                const emblem = document.createElement("span");
                emblem.className = "sow-hud__ring-emblem";
                bindAvatarFallback(image);
                renderAvatarIdentity(image, emblem, mapMenu.target_avatar);
                hub.append(image, emblem);
            } else hub.innerHTML = hudIcon("camera", "sow-hud__ring-icon");
        } else {
            hub = document.createElement("button");
            hub.type = "button";
            hub.dataset.mapBack = "true";
            hub.setAttribute("aria-label", SOW_t("hud.map_action_back"));
            hub.innerHTML =
                hudIcon(mapMenuView === "emote" ? "emote" : "construction", "sow-hud__ring-icon") +
                '<span class="sow-hud__ring-back" aria-hidden="true"></span>';
        }
        hub.className = "sow-hud__ring-hub";
        hub.dataset.relation = mapMenu.relation || "neutral";
        return hub;
    }

    function renderRingReadout(menu: HTMLElement): void {
        const readout = menu.querySelector<HTMLElement>(".sow-hud__ring-readout");
        if (!readout) return;
        const hud: HudRingState = hudState?.hud || {};
        const mapMenu = hud.map_menu || {};
        const slot = ring.hot >= 0 ? ring.slots[ring.hot] : null;
        let title, detail;
        if (slot) {
            title = ringSlotLabel(slot);
            detail = ringSlotDetail(slot, hud.gold);
        } else if (mapMenuView !== "root") {
            title = SOW_t(mapMenuView === "emote" ? "hud.map_action_emote" : "hud.map_action_build");
            detail = "";
        } else {
            const relation = SOW_t("hud.map_relation_" + (mapMenu.relation || "neutral"));
            title = mapMenu.target_name || relation;
            detail = title === relation ? "" : escapeHudText(relation);
        }
        readout.dataset.state = slot ? (ringSlotDisabled(slot) ? "blocked" : "action") : "target";
        const titleNode = readout.querySelector("strong");
        const detailNode = readout.querySelector("small");
        if (!titleNode || !detailNode) return;
        if (titleNode.textContent !== title) titleNode.textContent = title;
        if (detailNode.innerHTML !== detail) detailNode.innerHTML = detail;
        detailNode.hidden = !detail;
    }

    function buildRing(
        menu: HTMLElement,
        slots: (RingSlot | null)[],
        mapMenu: HudMapMenu,
        hud: HudRingState,
    ): void {
        menu.replaceChildren();
        menu.dataset.level = mapMenuView;
        menu.style.setProperty("--ring-hold-ms", RING_HOLD_MS + "ms");
        const readout = document.createElement("div");
        readout.className = "sow-hud__ring-readout";
        readout.setAttribute("aria-live", "polite");
        readout.append(document.createElement("strong"), document.createElement("small"));
        const radial = document.createElement("div");
        radial.className = "sow-hud__map-radial";
        radial.innerHTML =
            '<svg class="sow-hud__ring-art" viewBox="0 0 100 100" aria-hidden="true">' +
            '<defs><radialGradient id="sow-ring-sheen" cx="50" cy="50" r="50" gradientUnits="userSpaceOnUse">' +
            '<stop offset="0.4" stop-color="#fff" stop-opacity="0"/><stop offset="0.96" stop-color="#fff" stop-opacity="0.07"/>' +
            '<stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient></defs>' +
            '<circle class="sow-hud__ring-bezel" cx="50" cy="50" r="49.5"/>' +
            slots.map(ringSectorMarkup).join("") +
            '<circle class="sow-hud__ring-sheen" cx="50" cy="50" r="' +
            RING_OUTER_RADIUS +
            '" fill="url(#sow-ring-sheen)"/>' +
            '<circle class="sow-hud__ring-hub-plate" cx="50" cy="50" r="' +
            RING_HUB_RADIUS +
            '"/></svg>';
        slots.forEach(function (slot, index) {
            if (slot) radial.appendChild(ringSlotButton(slot, index, mapMenu, hud));
        });
        radial.appendChild(ringHub(mapMenu));
        const tether = document.createElement("span");
        tether.className = "sow-hud__ring-tether";
        tether.setAttribute("aria-hidden", "true");
        menu.append(readout, radial, tether);
    }

    // Live values (cost, reason, countdown) patch the built ring in place, so a
    // ticking timer never rebuilds it and hover/focus survive.
    function patchRing(
        menu: HTMLElement,
        slots: (RingSlot | null)[],
        availableGold: number | undefined,
        targetAvatar: HudAvatarIdentity | null | undefined = undefined,
    ): void {
        const image = menu.querySelector<HTMLImageElement>(".sow-hud__ring-portrait");
        const emblem = menu.querySelector<HTMLElement>(".sow-hud__ring-emblem");
        if (image && emblem) renderAvatarIdentity(image, emblem, targetAvatar);
        slots.forEach(function (slot, index) {
            if (!slot) return;
            const button = menu.querySelector<HTMLButtonElement>(
                '.sow-hud__ring-slot[data-slot="' + index + '"]',
            );
            if (!button) return;
            const disabled = ringSlotDisabled(slot);
            const name = ringSlotLabel(slot);
            const reason = slot.type === "action" ? mapActionReason(slot.item, availableGold) : "";
            const accessibleLabel = reason ? name + ". " + reason : name;
            if (button.disabled !== disabled) button.disabled = disabled;
            button.setAttribute("aria-disabled", String(disabled));
            if (button.getAttribute("aria-label") !== accessibleLabel)
                button.setAttribute("aria-label", accessibleLabel);
            let blocked = "";
            if (slot.type === "action" && slot.item.disabled)
                blocked = slot.item.reason_seconds != null ? "clock" : slot.item.reason_key ? "lock" : "gold";
            if (button.dataset.blocked !== blocked) {
                button.dataset.blocked = blocked;
                const badge = button.querySelector<HTMLElement>(".sow-hud__ring-badge");
                if (badge) badge.innerHTML = blocked ? hudIcon(blocked, "sow-hud__ring-badge-icon") : "";
            }
            const sector = menu.querySelector<HTMLElement>(
                '.sow-hud__ring-sector[data-slot="' + index + '"]',
            );
            if (sector) sector.classList.toggle("is-disabled", disabled);
        });
        ring.slots = slots;
        renderRingReadout(menu);
    }

    function positionRing(menu: HTMLElement, mapMenu: HudMapMenu): void {
        const x = Number(mapMenu.x || 0);
        const y = Number(mapMenu.y || 0);
        const half = menu.offsetWidth * 0.5;
        const readout = menu.querySelector<HTMLElement>(".sow-hud__ring-readout");
        const readoutSpace = readout ? readout.offsetHeight + 10 : 0;
        const minX = half + 8;
        const minY = half + 8;
        const maxX = Math.max(minX, window.innerWidth - half - 8);
        const maxY = Math.max(minY, window.innerHeight - half - 8);
        const left = Math.min(Math.max(x, minX), maxX);
        const top = Math.min(Math.max(y, minY), maxY);
        menu.style.left = left + "px";
        menu.style.top = top + "px";
        menu.dataset.readout = top - half - readoutSpace < 8 ? "below" : "above";
        const dx = x - left;
        const dy = y - top;
        const distance = Math.sqrt(dx * dx + dy * dy);
        menu.classList.toggle("has-tether", distance > half * 0.2);
        menu.style.setProperty("--tether-x", dx.toFixed(1) + "px");
        menu.style.setProperty("--tether-y", dy.toFixed(1) + "px");
        menu.style.setProperty("--tether-angle", Math.atan2(dy, dx).toFixed(4) + "rad");
        menu.style.setProperty("--tether-length", Math.max(0, distance - half).toFixed(1) + "px");
    }

    // Returns "hub", a slot index, or null. Direction picks the slot; while a
    // drag that opened the ring is in progress any distance past the hub counts.
    function ringSlotAt(
        menu: HTMLElement,
        clientX: number,
        clientY: number,
        unbounded: boolean,
    ): "hub" | number | null {
        const radial = menu.querySelector(".sow-hud__map-radial");
        if (!radial) return null;
        const rect = radial.getBoundingClientRect();
        if (!rect.width || !rect.height) return null;
        const dx = ((clientX - rect.left) / rect.width) * 100 - 50;
        const dy = ((clientY - rect.top) / rect.height) * 100 - 50;
        const distance = Math.sqrt(dx * dx + dy * dy);
        if (distance <= RING_HUB_RADIUS) return "hub";
        if (!unbounded && distance > 50) return null;
        const angle = ((Math.atan2(dx, -dy) * 180) / Math.PI + 360) % 360;
        return Math.round(angle / (360 / RING_SLOTS)) % RING_SLOTS;
    }

    function ringSlotButtonAt(menu: HTMLElement, index: number): HTMLButtonElement | null {
        return menu.querySelector<HTMLButtonElement>('.sow-hud__ring-slot[data-slot="' + index + '"]');
    }

    function setRingHot(menu: HTMLElement, index: number): void {
        if (ring.hold && ring.hold.index !== index) ringHoldCancel(menu);
        if (ring.hot === index) return;
        ring.hot = index;
        menu.querySelectorAll<HTMLElement>(".sow-hud__ring-sector, .sow-hud__ring-slot").forEach(
            function (element) {
                element.classList.toggle("is-hot", Number(element.dataset.slot) === index);
            },
        );
        renderRingReadout(menu);
    }

    function ringHoldStart(menu: HTMLElement, index: number): void {
        if (ring.hold && ring.hold.index === index) return;
        ringHoldCancel(menu);
        ring.hold = { index: index, started: performance.now() };
        const sector = menu.querySelector<HTMLElement>('.sow-hud__ring-sector[data-slot="' + index + '"]');
        if (sector) sector.classList.add("is-arming");
    }

    function ringHoldCancel(menu: HTMLElement | null): void {
        ring.hold = null;
        if (menu)
            menu.querySelectorAll<HTMLElement>(".sow-hud__ring-sector.is-arming").forEach(function (sector) {
                sector.classList.remove("is-arming");
            });
    }

    function ringActivate(menu: HTMLElement, target: "hub" | number | null): void {
        console.info("[RING TRACE] activate target=" + String(target));
        if (target === "hub") {
            // The center is "back" inside a submenu and "center on the entity's home, then close" at the root.
            const back = menu.querySelector<HTMLButtonElement>("[data-map-back]");
            if (back) back.click();
            else {
                send("center_radial_home");
                send("close_map_context_menu");
            }
            return;
        }
        if (typeof target !== "number") return;
        setRingHot(menu, target);
        const button = ringSlotButtonAt(menu, target);
        if (!button || button.disabled) return;
        if (button.dataset.ringHold === "true") {
            const held =
                ring.hold &&
                ring.hold.index === target &&
                performance.now() - ring.hold.started >= RING_HOLD_MS;
            ringHoldCancel(menu);
            if (!held) return;
            ring.holdConfirmed = true;
            button.click();
            ring.holdConfirmed = false;
            return;
        }
        button.click();
    }

    function renderMapMenu(mapMenu: HudMapMenu | null | undefined): boolean {
        if (!hudRefs || !hudRefs.mapMenu) return false;
        const menu = hudRefs.mapMenu;
        const open = Boolean(mapMenu && mapMenu.open);
        if (open && mapMenu && mapMenu.view === "building_details") {
            ring.session = "";
            mapMenuView = "root";
            ring.hot = -1;
            ring.press = null;
            ringHoldCancel(null);
            return renderBuildingDetailsCard(menu, mapMenu, hudState?.hud || {});
        }
        menu.classList.remove("is-building-card");
        delete menu.dataset.placement;
        const showRadial = Boolean(open && mapMenu && mapMenu.view === "radial");
        menu.classList.toggle("hidden", !showRadial);
        if (!showRadial) {
            if (menu.dataset.renderKey) menu.replaceChildren();
            menu.dataset.renderKey = "";
            delete menu.dataset.level;
            ring.session = "";
            mapMenuView = "root";
            ring.hot = -1;
            ring.press = null;
            ringHoldCancel(null);
            return false;
        }
        if (!mapMenu) return false;
        const hud: HudRingState = hudState?.hud || {};
        const session = String(mapMenu.session) + ":" + String(mapMenu.tile_idx);
        if (ring.session !== session) {
            ring.session = session;
            menu.dataset.session = String(mapMenu.session);
            menu.dataset.tileIdx = String(mapMenu.tile_idx);
            mapMenuView = "root";
            ring.hot = -1;
            ringHoldCancel(null);
            ring.press = ring.pointerDown ? { id: ring.pointerId, fromOpen: true } : null;
        }
        const items = mapItems(mapMenu);
        let slots = ringSlots(mapMenu, items, hud);
        if (mapMenuView !== "root" && !slots.some(Boolean)) {
            mapMenuView = "root";
            slots = ringSlots(mapMenu, items, hud);
        }
        ring.slots = slots;
        const renderKey =
            [
                window.SOW_LOCALE || "en",
                session,
                mapMenuView,
                mapMenu.alliance_state,
                mapMenu.relation,
                mapMenu.target_player_id,
                hud.emote_active,
                hud.emote_pinned,
                hud.player_leader,
                mapMenu.building && mapMenu.building.level,
            ].join(":") +
            ":" +
            slots.map(ringSlotIdentity).join(",");
        if (menu.dataset.renderKey !== renderKey) {
            buildRing(menu, slots, mapMenu, hud);
            menu.dataset.renderKey = renderKey;
            const hot = ring.hot;
            ring.hot = -1;
            if (hot >= 0 && slots[hot]) setRingHot(menu, hot);
        }
        patchRing(menu, slots, hud.gold, mapMenu.target_avatar);
        positionRing(menu, mapMenu);
        return true;
    }

    function openRingMenu(): HTMLElement | null {
        const menu = hudRefs && hudRefs.mapMenu;
        return menu && !menu.classList.contains("hidden") && menu.dataset.level ? menu : null;
    }

    function ringPointer(menu: HTMLElement, event: PointerEvent): void {
        const target = ringSlotAt(
            menu,
            event.clientX,
            event.clientY,
            Boolean(ring.press && ring.press.fromOpen),
        );
        const index = typeof target === "number" ? target : -1;
        if (event.pointerType === "mouse" || ring.press) setRingHot(menu, index);
        if (!ring.press || index < 0) return;
        const button = ringSlotButtonAt(menu, index);
        if (button && !button.disabled && button.dataset.ringHold === "true") ringHoldStart(menu, index);
    }

    function bindRingInput() {
        window.addEventListener(
            "pointerdown",
            function (event) {
                ring.pointerDown = true;
                ring.pointerId = event.pointerId;
                const menu = openRingMenu();
                if (!menu || !menu.contains(event.target as Node)) return;
                event.preventDefault();
                ring.press = { id: event.pointerId, fromOpen: false };
                ringPointer(menu, event);
            },
            true,
        );
        window.addEventListener(
            "pointermove",
            function (event) {
                const menu = openRingMenu();
                if (menu) ringPointer(menu, event);
            },
            true,
        );
        window.addEventListener(
            "pointerup",
            function (event) {
                ring.pointerDown = false;
                const menu = openRingMenu();
                const press = ring.press;
                ring.press = null;
                console.info(
                    "[RING TRACE] pointerup button=" +
                        event.button +
                        " type=" +
                        event.pointerType +
                        " menu=" +
                        Boolean(menu) +
                        " fromOpen=" +
                        (press ? press.fromOpen : "none"),
                );
                if (!menu || !press || press.id !== event.pointerId) {
                    if (menu) ringHoldCancel(menu);
                    return;
                }
                const target = ringSlotAt(menu, event.clientX, event.clientY, press.fromOpen);
                // Releasing the opening press on the hub leaves the ring open for taps.
                if (press.fromOpen && typeof target !== "number") return;
                ringActivate(menu, target);
            },
            true,
        );
        window.addEventListener(
            "pointercancel",
            function () {
                ring.pointerDown = false;
                ring.press = null;
                const menu = openRingMenu();
                if (menu) ringHoldCancel(menu);
            },
            true,
        );
    }

    function escapeHudText(value: string | number | boolean | null | undefined): string {
        return String(value).replace(/[&<>"']/g, function (character) {
            return (
                { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[character] ||
                character
            );
        });
    }

    function devColorHex(value: number[], linear: boolean): string {
        function channel(v: number): string {
            v = Math.max(0, Math.min(1, Number(v) || 0));
            if (linear) v = v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055;
            return Math.round(v * 255)
                .toString(16)
                .padStart(2, "0");
        }
        return "#" + channel(value[0]) + channel(value[1]) + channel(value[2]);
    }

    function devColorFromHex(hex: string, linear: boolean): number[] {
        const value = [1, 3, 5].map(function (offset) {
            const v = parseInt(hex.slice(offset, offset + 2), 16) / 255;
            return linear ? (v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4)) : v;
        });
        return value;
    }

    const DEV_RESET_ICON =
        '<svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 12a8 8 0 1 0 2.6-5.9"/><path d="M4 4v5h5"/></svg>';

    function devDecimals(step: number): number {
        const text = String(step);
        const dot = text.indexOf(".");
        return dot < 0 ? 0 : Math.min(2, text.length - dot - 1);
    }

    // At most 2 decimals; a nonzero value never reads as 0.00.
    function devFormat(value: number | string, decimals: number): string {
        const number = Number(value);
        if (!Number.isFinite(number)) return "";
        const text = number.toFixed(decimals);
        return number !== 0 && Number(text) === 0 ? number.toFixed(3) : text;
    }

    function devIsDirty(def: HudDevParameter, current: boolean | number | number[] | undefined): boolean {
        const now = Array.isArray(current) ? current : [current];
        const base = Array.isArray(def.default) ? def.default : [def.default];
        return now.some(function (part, index) {
            return Math.abs(Number(part) - Number(base[index])) > 1e-6;
        });
    }

    function sendDevControl(input: HTMLInputElement): void {
        const control = input.closest<HTMLElement>("[data-dev-control]");
        if (!control) return;
        const id = control.dataset.devControl;
        const kind = control.dataset.devKind;
        if (!id || !kind) return;
        const decimals = Number(control.dataset.devDecimals) || 0;
        let value: boolean | number | number[];
        if (kind === "bool") {
            value = input.checked;
        } else if (kind === "number" || kind === "select") {
            value = Number(input.value);
            if (!Number.isFinite(value)) return;
            if (kind === "number") {
                const range = control.querySelector<HTMLInputElement>('input[type="range"]');
                if (range && input.type !== "range") {
                    const rangeMin = Number(range.dataset.devMin);
                    const rangeMax = Number(range.dataset.devMax);
                    const margin = Math.max((rangeMax - rangeMin) * 0.1, Number(range.step) || 0.01);
                    range.min = String(value < rangeMin ? value - margin : rangeMin);
                    range.max = String(value > rangeMax ? value + margin : rangeMax);
                }
                control
                    .querySelectorAll<HTMLInputElement>('[data-dev="' + id + '"]')
                    .forEach(function (sibling) {
                        if (sibling !== input)
                            sibling.value =
                                sibling.type === "range" ? input.value : devFormat(input.value, decimals);
                    });
            }
        } else {
            const config = hudState?.hud?.dev_tools?.config || {};
            const configuredValue = config[id];
            const vector = Array.isArray(configuredValue)
                ? configuredValue.slice()
                : kind === "color"
                  ? [1, 1, 1, 1]
                  : [0, 0, 0];
            value = vector;
            if (input.hasAttribute("data-dev-picker")) {
                const rgb = devColorFromHex(input.value, input.dataset.devLinear === "1");
                vector[0] = rgb[0];
                vector[1] = rgb[1];
                vector[2] = rgb[2];
                control.querySelectorAll<HTMLInputElement>("[data-dev-channel]").forEach(function (channel) {
                    const channelIndex = Number(channel.dataset.devChannel);
                    if (channelIndex < 3) channel.value = String(vector[channelIndex]);
                });
            } else {
                const index = Number(input.dataset.devChannel);
                vector[index] = Number(input.value);
                if (!Number.isFinite(vector[index])) return;
                devReadout(input, decimals);
                const picker = control.querySelector<HTMLInputElement>("[data-dev-picker]");
                if (picker && index < 3) picker.value = devColorHex(vector, picker.dataset.devLinear === "1");
            }
        }
        send("set_dev_config", { field: id, value: value });
    }

    function disarmDevReset() {
        window.clearTimeout(devResetTimer);
        devResetTimer = 0;
        const button =
            hudRefs && hudRefs.devSidebar && hudRefs.devSidebar.querySelector("#sow-hud-dev-reset-all");
        if (button) button.textContent = "Reset all";
    }

    function setDevMenu(open: boolean): void {
        disarmDevReset();
        const menu = hudRefs!.devSidebar.querySelector<HTMLElement>("#sow-hud-dev-menu");
        const trigger = hudRefs!.devSidebar.querySelector<HTMLElement>('[data-command="dev_menu"]');
        if (menu) menu.hidden = !open;
        if (trigger) trigger.setAttribute("aria-expanded", open ? "true" : "false");
    }

    function devReadout(input: HTMLInputElement, decimals: number): void {
        const readout = input.parentElement && input.parentElement.querySelector("output");
        if (readout) readout.textContent = devFormat(input.value, decimals);
    }

    function devStepOnce(button: HTMLButtonElement): void {
        const control = button.closest<HTMLElement>("[data-dev-control]");
        const range = control && control.querySelector<HTMLInputElement>('input[type="range"]');
        const chip = control && control.querySelector<HTMLInputElement>('input[type="text"][data-dev]');
        if (!range || !chip) return;
        const next = Number(range.value) + Number(button.dataset.devStepDir) * (Number(range.step) || 0.01);
        if (!Number.isFinite(next)) return;
        chip.value = String(Number(next.toFixed(6)));
        sendDevControl(chip);
        chip.value = devFormat(next, Number(control.dataset.devDecimals) || 0);
    }

    function devControlMarkup(def: HudDevParameter): string {
        const id = escapeHudText(def.id);
        const label = escapeHudText(def.label);
        const decimals = def.kind === "color" || def.kind === "vector" ? 2 : devDecimals(def.step);
        const attrs =
            ' data-dev-control="' +
            id +
            '" data-dev-kind="' +
            def.kind +
            '" data-dev-decimals="' +
            decimals +
            '" data-dev-search-text="' +
            escapeHudText((def.section + " " + def.label + " " + def.id).toLowerCase()) +
            '"';
        const name = '<span class="sow-hud__dev-label">' + label + "</span>";
        const reset =
            '<button type="button" class="sow-hud__dev-reset" data-command="reset_dev_config" data-dev-field="' +
            id +
            '" aria-label="Reset ' +
            label +
            '">' +
            DEV_RESET_ICON +
            "</button>";
        const channel = function (
            text: string,
            index: number,
            min: number,
            max: number,
            step: number,
            tone: string,
            readout = false,
        ): string {
            return (
                '<label class="sow-hud__dev-channel" data-tone="' +
                tone +
                '"><b>' +
                text +
                "</b>" +
                '<input type="range" min="' +
                min +
                '" max="' +
                max +
                '" step="' +
                step +
                '" data-dev="' +
                id +
                '" data-dev-channel="' +
                index +
                '" aria-label="' +
                label +
                " " +
                text +
                '">' +
                (readout ? "<output></output>" : "") +
                "</label>"
            );
        };
        if (def.kind === "bool") {
            return (
                '<label class="sow-hud__dev-control sow-hud__dev-control--bool"' +
                attrs +
                ">" +
                name +
                reset +
                '<input class="sow-hud__switch" type="checkbox" data-dev="' +
                id +
                '"></label>'
            );
        }
        let html = '<div class="sow-hud__dev-control"' + attrs + '><div class="sow-hud__dev-head">' + name;
        if (def.kind === "number") {
            const unit = ({ s: "s", "screen px": "px" } as Record<string, string>)[def.unit];
            html +=
                '<input class="sow-hud__dev-chip-input" type="text" inputmode="decimal" autocomplete="off" data-dev="' +
                id +
                '" aria-label="' +
                label +
                ' value">' +
                (unit ? '<span class="sow-hud__dev-unit">' + unit + "</span>" : "") +
                reset +
                "</div>" +
                '<div class="sow-hud__dev-slide"><button type="button" data-command="dev_step" data-dev-step-dir="-1" aria-label="Decrease ' +
                label +
                '">−</button>' +
                '<input type="range" min="' +
                def.min +
                '" max="' +
                def.max +
                '" data-dev-min="' +
                def.min +
                '" data-dev-max="' +
                def.max +
                '" step="' +
                def.step +
                '" data-dev="' +
                id +
                '" aria-label="' +
                label +
                '">' +
                '<button type="button" data-command="dev_step" data-dev-step-dir="1" aria-label="Increase ' +
                label +
                '">+</button></div>';
        } else if (def.kind === "select") {
            html +=
                reset +
                '</div><div class="sow-hud__dev-seg" role="radiogroup" aria-label="' +
                label +
                '">' +
                (def.options || [])
                    .map(function (option) {
                        return (
                            '<label><input type="radio" name="dev-' +
                            id +
                            '" value="' +
                            option.value +
                            '" data-dev="' +
                            id +
                            '"><span>' +
                            escapeHudText(option.label) +
                            "</span></label>"
                        );
                    })
                    .join("") +
                "</div>";
        } else if (def.kind === "color") {
            html +=
                '<input class="sow-hud__dev-color-picker" type="color" data-dev-picker="' +
                id +
                '" data-dev-linear="' +
                (def.unit === "linear RGB" ? "1" : "0") +
                '" aria-label="' +
                label +
                ' color">' +
                reset +
                "</div>" +
                channel("R", 0, def.min, def.max, def.step, "r") +
                channel("G", 1, def.min, def.max, def.step, "g") +
                channel("B", 2, def.min, def.max, def.step, "b") +
                (def.alpha ? channel("A", 3, 0, 1, 0.01, "a") : "");
        } else {
            html +=
                reset +
                "</div>" +
                channel("X", 0, def.min, def.max, def.step, "x", true) +
                channel("Y", 1, def.min, def.max, def.step, "x", true) +
                channel("Z", 2, def.min, def.max, def.step, "x", true);
        }
        return html + "</div>";
    }

    function buildDevPanel(devTools: CompleteHudDevTools, sidebar: HTMLElement, host: HTMLElement): void {
        const bySection: Record<string, NonNullable<HudDevTools["schema"]>> = Object.create(null);
        devTools.schema.forEach(function (def) {
            (bySection[def.section] || (bySection[def.section] = [])).push(def);
        });
        devControlIndex = Object.create(null);
        let tabs = "";
        let html = "";
        devTools.groups.forEach(function (group) {
            const groupName = escapeHudText(group.name);
            tabs +=
                '<button type="button" role="tab" data-command="dev_tab" data-dev-tab="' +
                groupName +
                '">' +
                groupName +
                "<i></i></button>";
            group.sections.forEach(function (section, index) {
                const defs = bySection[section.name];
                if (!defs) return;
                const sectionName = escapeHudText(section.name);
                html +=
                    '<details class="sow-hud__dev-section" data-dev-group="' +
                    groupName +
                    '" data-dev-name="' +
                    sectionName +
                    '"' +
                    (index === 0 ? " open" : "") +
                    "><summary>" +
                    '<span class="sow-hud__dev-section-name">' +
                    sectionName +
                    "</span>" +
                    (section.preview
                        ? '<button type="button" class="sow-hud__dev-play" data-command="dev_preview" data-dev-target="' +
                          section.preview +
                          '" aria-label="Preview ' +
                          sectionName +
                          '"></button>'
                        : "") +
                    '<button type="button" class="sow-hud__dev-section-reset" data-command="reset_dev_config" data-dev-section="' +
                    sectionName +
                    '">Reset</button></summary>' +
                    defs.map(devControlMarkup).join("") +
                    "</details>";
            });
        });
        if (Array.isArray(devTools.perf) && devTools.perf.length) {
            tabs +=
                '<button type="button" role="tab" data-command="dev_tab" data-dev-tab="Perf">Perf<i></i></button>';
        }
        const tabRoot = sidebar.querySelector<HTMLElement>("#sow-hud-dev-tabs");
        if (tabRoot) tabRoot.innerHTML = tabs;
        host.innerHTML = html;
        host.querySelectorAll<HTMLElement>("[data-dev-control]").forEach(function (control) {
            const id = control.dataset.devControl;
            if (id) devControlIndex[id] = control;
        });
    }

    function renderDevControls(devTools: Partial<HudDevTools>): void {
        const sidebar = hudRefs && hudRefs.devSidebar;
        const host = sidebar && sidebar.querySelector<HTMLElement>("#sow-hud-dev-controls");
        if (!host || !devTools.schema || !devTools.groups) return;
        const completeDevTools: CompleteHudDevTools = {
            open: devTools.open === true,
            config: devTools.config,
            schema: devTools.schema,
            groups: devTools.groups,
            perf: devTools.perf,
            preview: devTools.preview,
        };
        const key =
            completeDevTools.schema
                .map(function (def) {
                    return [
                        def.id,
                        def.kind,
                        def.section,
                        def.label,
                        def.min,
                        def.max,
                        def.step,
                        def.unit,
                    ].join(":");
                })
                .join("|") +
            "#" +
            JSON.stringify(completeDevTools.groups) +
            (Array.isArray(completeDevTools.perf) && completeDevTools.perf.length ? "#perf" : "");
        if (key !== devToolsSchemaKey) {
            buildDevPanel(completeDevTools, sidebar, host);
            devToolsSchemaKey = key;
        }
        const perfHost = sidebar.querySelector<HTMLElement>("#sow-hud-dev-perf");
        const perf = Array.isArray(completeDevTools.perf) ? completeDevTools.perf : [];
        if (perfHost) {
            if (
                perfHost.children.length !== perf.length ||
                perf.some(function (item, index) {
                    return (
                        perfHost.querySelectorAll<HTMLButtonElement>("button[data-dev-perf]")[index]?.dataset
                            .devPerf !== item.name
                    );
                })
            ) {
                perfHost.innerHTML = perf
                    .map(function (item) {
                        const name = escapeHudText(item.name);
                        return (
                            '<button type="button" data-command="dev_perf" data-dev-perf="' +
                            name +
                            '">' +
                            name +
                            "</button>"
                        );
                    })
                    .join("");
            }
            perfHost
                .querySelectorAll<HTMLButtonElement>("button[data-dev-perf]")
                .forEach(function (button, index) {
                    const item = perf[index];
                    if (!item) return;
                    if (button.dataset.state !== item.state) button.dataset.state = item.state;
                    button.setAttribute(
                        "aria-pressed",
                        item.state === "on" ? "true" : item.state === "mixed" ? "mixed" : "false",
                    );
                });
        }
        const tabs = Array.from(sidebar.querySelectorAll<HTMLElement>("[data-dev-tab]"));
        if (
            !tabs.some(function (tab) {
                return tab.dataset.devTab === devTab;
            })
        ) {
            devTab = tabs[0]?.dataset.devTab || "";
            try {
                const saved = window.localStorage.getItem("sow_dev_tab");
                if (
                    saved &&
                    tabs.some(function (tab) {
                        return tab.dataset.devTab === saved;
                    })
                )
                    devTab = saved;
            } catch {
                /* storage unavailable */
            }
        }
        const search = sidebar.querySelector<HTMLInputElement>("[data-dev-search]");
        const query = search ? search.value.trim().toLowerCase() : "";
        const perfTab = devTab === "Perf" && query === "";
        if (perfHost) perfHost.hidden = !perfTab;
        tabs.forEach(function (tab) {
            const selected = query === "" && tab.dataset.devTab === devTab;
            tab.classList.toggle("is-active", selected);
            tab.setAttribute("aria-selected", selected ? "true" : "false");
        });
        const config = completeDevTools.config || {};
        const dirtyBySection: Record<string, number> = Object.create(null);
        completeDevTools.schema.forEach(function (def) {
            const control = devControlIndex[def.id];
            if (!control) return;
            const current = config[def.id];
            const dirty = devIsDirty(def, current);
            if (dirty) dirtyBySection[def.section] = (dirtyBySection[def.section] || 0) + 1;
            const flag = dirty ? "true" : "false";
            if (control.dataset.dirty !== flag) control.dataset.dirty = flag;
            if (def.id.indexOf("water.") === 0 && /_limit$/.test(def.id)) {
                control.dataset.warn = Number(current) > Number(config["water.depth_cap"]) ? "true" : "false";
            }
            if (def.kind === "number") {
                const range = control.querySelector<HTMLInputElement>('input[type="range"]');
                const active = document.activeElement;
                const editingControl =
                    active &&
                    active.matches &&
                    active.matches("[data-dev]") &&
                    active.closest("[data-dev-control]") === control;
                if (range && !editingControl) {
                    const value = Number(current);
                    const min = Number(range.dataset.devMin);
                    const max = Number(range.dataset.devMax);
                    const margin = Math.max((max - min) * 0.1, Number(range.step) || 0.01);
                    range.min = String(value < min ? value - margin : min);
                    range.max = String(value > max ? value + margin : max);
                }
            }
        });
        const dirtyByGroup: Record<string, boolean> = Object.create(null);
        host.querySelectorAll<HTMLDetailsElement>(".sow-hud__dev-section").forEach(function (section) {
            const sectionName = section.dataset.devName;
            const groupName = section.dataset.devGroup;
            if (!sectionName || !groupName) return;
            const count = dirtyBySection[sectionName] || 0;
            section.dataset.dirty = count ? "true" : "false";
            if (count) dirtyByGroup[groupName] = true;
            let matches = 0;
            section.querySelectorAll<HTMLElement>("[data-dev-control]").forEach(function (control) {
                const hide = query !== "" && control.dataset.devSearchText!.indexOf(query) < 0;
                control.hidden = hide;
                if (!hide) matches++;
            });
            section.hidden = query !== "" ? matches === 0 : perfTab || section.dataset.devGroup !== devTab;
            if (query && matches) section.open = true;
        });
        dirtyByGroup.Perf = perf.some(function (item) {
            return item.state !== "on";
        });
        tabs.forEach(function (tab) {
            const tabName = tab.dataset.devTab;
            if (tabName) tab.dataset.dirty = dirtyByGroup[tabName] ? "true" : "false";
        });
        const preview = completeDevTools.preview || {
            target: null,
            phase: 0,
            paused: false,
            repeat: false,
            active: false,
            playing: false,
        };
        const transport = sidebar.querySelector<HTMLElement>("#sow-hud-dev-transport");
        if (transport) transport.hidden = !preview.active;
        sidebar.querySelectorAll<HTMLInputElement>("[data-dev-preview]").forEach(function (input) {
            if (document.activeElement === input) return;
            const field = input.dataset.devPreview;
            if (!field) return;
            const previewValues: Record<string, string | number | boolean | null> = {
                target: preview.target,
                phase: preview.phase,
                paused: preview.paused,
                repeat: preview.repeat,
            };
            const value = previewValues[field];
            if (value == null) return;
            if (input.type === "checkbox") input.checked = Boolean(value);
            else input.value = String(value);
        });
        sidebar.querySelectorAll<HTMLButtonElement>(".sow-hud__dev-play").forEach(function (button) {
            button.classList.toggle(
                "is-playing",
                Boolean(preview.active) && Number(button.dataset.devTarget) === preview.target,
            );
        });
    }

    // Canonical trade/transfer icon for leaderboard actions.
    // Prepared so it can be swapped for a custom SVG template or asset directly in one place.
    const LEADERBOARD_TRADE_CUSTOM_SVG: string | null = null;

    function leaderboardTradeIcon(cls?: string): string {
        if (LEADERBOARD_TRADE_CUSTOM_SVG) {
            return LEADERBOARD_TRADE_CUSTOM_SVG;
        }
        return hudIcon("transfer", cls || "sow-hud__btn-icon");
    }

    function leaderboardGpsIcon(cls?: string): string {
        return hudIcon("camera", cls || "sow-hud__btn-icon");
    }

    function leaderboardPlayerAvatar(player: HudLeaderboardPlayer): HudAvatarIdentity {
        if (player.avatar && typeof player.avatar === "string") {
            return {
                kind: "portrait",
                slug: player.avatar,
                expression: player.avatar_expression || undefined,
                transition: player.avatar_transition,
            };
        }
        const leader = leaderById(player.leader);
        const slug = (leader && (leader.art_slug || leader.slug)) || "null";
        return {
            kind: "portrait",
            slug: slug,
            expression: player.avatar_expression || undefined,
            transition: player.avatar_transition,
        };
    }

    function applyLeaderboardFilter(): void {
        if (!hudRefs || !hudRefs.rows) return;
        const q = leaderboardFilterQuery;
        let visibleCount = 0;
        const cards = Array.from(hudRefs.rows.querySelectorAll<HTMLElement>(".sow-hud__player-card"));
        for (const card of cards) {
            const name = (card.querySelector(".sow-hud__player-name")?.textContent || "").toLowerCase();
            const rank = (card.querySelector(".sow-hud__player-rank")?.textContent || "").toLowerCase();
            const match = !q || name.includes(q) || rank.includes(q);
            card.hidden = !match;
            if (match) visibleCount++;
        }
        if (hudRefs.lbEmpty) {
            hudRefs.lbEmpty.hidden = visibleCount > 0 || !q;
        }
    }

    function updateLeaderboard(players: HudLeaderboardPlayer[], teams: HudLeaderboardTeam[]): void {
        if (!hudRefs || !hudRefs.rows) return;
        if (Array.isArray(players)) {
            lastLeaderboardPlayers = players;
        } else {
            players = lastLeaderboardPlayers;
        }
        if (Array.isArray(teams)) {
            lastLeaderboardTeams = teams;
        } else {
            teams = lastLeaderboardTeams;
        }
        if (!Array.isArray(players)) return;
        const teamRenderKey = teams
            .map(function (team, idx) {
                return [idx, team.team, team.tile_count, team.territory_pct, team.members].join("|");
            })
            .join("\u001e");
        const playerRenderKey = players
            .map(function (player, idx) {
                return [
                    idx,
                    player.id,
                    player.rank,
                    player.name,
                    player.troops,
                    player.tile_count,
                    player.territory_pct,
                    player.is_alive,
                    player.is_me,
                    player.leader,
                    player.avatar,
                    player.avatar_expression,
                    JSON.stringify(player.avatar_transition),
                ].join("|");
            })
            .join("\u001e");
        const renderKey = teamRenderKey + "\u001d" + playerRenderKey;
        if (renderKey === leaderboardRenderKey) return;
        const nextRows = Object.create(null);
        const fragment = document.createDocumentFragment();
        teams.forEach(function (team, idx) {
            const card = document.createElement("div");
            const rank = document.createElement("span");
            const name = document.createElement("b");
            const territory = document.createElement("span");

            card.className = "sow-hud__player-card sow-hud__team-summary";
            rank.className = "sow-hud__player-rank";
            name.className = "sow-hud__player-name";
            territory.className = "sow-hud__player-land";
            rank.textContent = "#" + (idx + 1);
            name.textContent = SOW_t(team.team === "Red" ? "lobbies.red_team" : "lobbies.blue_team", {
                count: team.members,
            });
            territory.textContent = Math.round((team.territory_pct || 0) * 100) + "%";
            card.append(rank, name, territory);
            fragment.appendChild(card);
        });
        (players || []).forEach(function (player, idx) {
            const key = String(player.id);
            let row = leaderboardRows[key];
            if (!row) {
                const card = document.createElement("div");
                const left = document.createElement("div");
                const rank = document.createElement("span");
                const avatarWrap = document.createElement("span");
                const avatarImg = document.createElement("img");
                const avatarBadge = document.createElement("span");
                const nameWrap = document.createElement("div");
                const name = document.createElement("b");
                const meta = document.createElement("div");
                const stats = document.createElement("div");
                const territory = document.createElement("span");
                const troops = document.createElement("span");
                const actions = document.createElement("div");
                const gps = document.createElement("button");
                const transfer = document.createElement("button");

                card.className = "sow-hud__player-card";
                card.dataset.command = "focus_player";
                card.dataset.playerId = String(player.id);

                left.className = "sow-hud__player-identity";
                rank.className = "sow-hud__player-rank";

                avatarWrap.className = "sow-hud__player-avatar-wrap";
                avatarImg.className = "sow-hud__player-avatar";
                avatarImg.alt = "";
                avatarImg.draggable = false;
                bindAvatarFallback(avatarImg);

                avatarBadge.className = "sow-hud__player-avatar-badge";
                avatarBadge.hidden = true;
                avatarWrap.append(avatarImg, avatarBadge);

                nameWrap.className = "sow-hud__player-name-wrap";
                name.className = "sow-hud__player-name";
                nameWrap.appendChild(name);

                left.append(rank, avatarWrap, nameWrap);

                meta.className = "sow-hud__player-meta";
                stats.className = "sow-hud__player-stats";
                territory.className = "sow-hud__player-land";
                troops.className = "sow-hud__player-troops";
                stats.append(territory, troops);

                actions.className = "sow-hud__player-actions";

                gps.type = "button";
                gps.className = "sow-hud__row-action sow-hud__row-action--icon sow-hud__row-action--gps";
                gps.dataset.command = "focus_player";
                gps.dataset.playerId = String(player.id);
                gps.setAttribute("aria-label", SOW_t("hud.center_camera") || "Locate");
                gps.title = SOW_t("hud.center_camera") || "Locate";
                gps.innerHTML = leaderboardGpsIcon();

                transfer.type = "button";
                transfer.className =
                    "sow-hud__row-action sow-hud__row-action--icon sow-hud__row-action--trade";
                transfer.dataset.command = "open_transfer";
                transfer.dataset.playerId = String(player.id);
                transfer.setAttribute("aria-label", SOW_t("hud.map_action_transfer") || "Trade");
                transfer.title = SOW_t("hud.map_action_transfer") || "Trade";
                transfer.innerHTML = leaderboardTradeIcon();

                actions.append(gps, transfer);
                meta.append(stats, actions);

                card.append(left, meta);

                row = {
                    card: card,
                    rank: rank,
                    avatarWrap: avatarWrap,
                    avatarImg: avatarImg,
                    avatarBadge: avatarBadge,
                    name: name,
                    territory: territory,
                    troops: troops,
                    gpsBtn: gps,
                    transferBtn: transfer,
                    key: "",
                };
            }

            const displayName = player.name || SOW_t("hud.player_name");
            const rowKey = [
                player.rank,
                idx,
                displayName,
                player.troops,
                player.tile_count,
                player.territory_pct,
                player.is_alive,
                player.is_me,
                player.leader,
                player.avatar,
                player.avatar_expression,
                JSON.stringify(player.avatar_transition),
            ].join("|");
            if (row.key !== rowKey) {
                row.key = rowKey;
                row.card.classList.toggle("is-me", !!player.is_me);
                row.card.classList.toggle("is-dead", !player.is_alive);
                const displayRank = Number.isFinite(Number(player.rank)) ? Number(player.rank) : idx + 1;
                row.card.classList.toggle("is-top-1", displayRank === 1);
                row.card.classList.toggle("is-top-2", displayRank === 2);
                row.card.classList.toggle("is-top-3", displayRank === 3);
                row.rank.textContent = "#" + displayRank;
                row.name.textContent = displayName;

                renderAvatarIdentity(row.avatarImg, row.avatarBadge, leaderboardPlayerAvatar(player));

                row.territory.textContent = Math.round((player.territory_pct || 0) * 100) + "%";
                row.troops.innerHTML =
                    escapeHudText(
                        player.troops > 1000
                            ? (player.troops / 1000).toFixed(1) + "k"
                            : Math.floor(player.troops),
                    ) +
                    " " +
                    hudIcon("troops", "sow-hud__inline-icon");

                row.gpsBtn.dataset.playerId = String(player.id);
                row.transferBtn.dataset.playerId = String(player.id);
                row.transferBtn.disabled = !player.is_alive || !!player.is_me;
                row.gpsBtn.disabled = !player.is_alive;
            }
            fragment.appendChild(row.card);
            nextRows[key] = row;
        });
        hudRefs.rows.replaceChildren(fragment);
        leaderboardRows = nextRows;
        leaderboardRenderKey = renderKey;
        applyLeaderboardFilter();
    }

    function createNotificationCard(container: HTMLElement, contextual: boolean): HudNotificationCard {
        const card = document.createElement("div");
        card.className =
            "sow-hud__notification sow-hud__notification--premium" +
            (contextual ? " sow-hud__notification--contextual" : "");
        card.hidden = true;
        const seal = document.createElement("span");
        seal.className = "sow-hud__notification-seal";
        seal.setAttribute("aria-hidden", "true");
        const portraits = document.createElement("span");
        portraits.className = "sow-hud__notification-portraits";
        const avatars = [];
        for (let index = 0; index < 2; index += 1) {
            const wrapper = document.createElement("span");
            wrapper.className = "sow-hud__notification-avatar";
            wrapper.hidden = true;
            const image = document.createElement("img");
            image.className = "sow-hud__notification-portrait";
            image.alt = "";
            image.draggable = false;
            image.hidden = true;
            bindAvatarFallback(image);
            const emblem = document.createElement("span");
            emblem.className = "sow-hud__notification-emblem";
            emblem.hidden = true;
            wrapper.append(image, emblem);
            portraits.appendChild(wrapper);
            avatars.push({ wrapper: wrapper, image: image, emblem: emblem });
        }
        const copy = document.createElement("span");
        copy.className = "sow-hud__notification-copy";
        card.append(seal, portraits, copy);
        container.appendChild(card);
        return { card: card, seal: seal, avatars: avatars, copy: copy, renderKey: "" };
    }

    function bindAvatarFallback(image: HTMLImageElement): void {
        image.onerror = function () {
            const fallback = asset("gameplay/avatars/null.webp");
            if (this.getAttribute("src") !== fallback) {
                this.hidden = false;
                this.src = fallback;
            } else {
                this.hidden = true;
                this.removeAttribute("src");
            }
        };
    }

    function avatarPortraitPath(
        slug: string,
        expression: HudAvatarIdentity["expression"] | null | undefined,
    ): string {
        const path =
            expression && expression !== "calm" && ["happy", "fear", "angry"].includes(expression)
                ? "expressions/" + slug + "_" + expression
                : slug;
        return "gameplay/avatars/" + path + ".webp";
    }

    function renderAvatarIdentity(
        image: HTMLImageElement,
        emblem: HTMLElement,
        presentation: HudAvatarIdentity | null | undefined,
    ): void {
        const reducedMotion = Boolean(
            typeof document !== "undefined" &&
            typeof document.getElementById === "function" &&
            document.getElementById("sow-hud")?.classList.contains("is-reduced-motion"),
        );
        const identityKey = (presentation ? JSON.stringify(presentation) : "") + ":" + reducedMotion;
        if (avatarIdentityKeys.get(image) === identityKey) return;
        avatarIdentityKeys.set(image, identityKey);
        emblem.hidden = true;
        emblem.textContent = "";
        if (!presentation) {
            image.hidden = true;
            setAvatarPortrait(image, "", null, true);
            return;
        }

        if (presentation.kind === "wilderness") {
            image.hidden = true;
            setAvatarPortrait(image, "", null, true);
            emblem.innerHTML = statusIcon("wilderness", "sow-hud__avatar-badge");
            emblem.hidden = false;
            return;
        }

        const slug =
            presentation.kind === "portrait" && /^[a-z][a-z0-9_]*$/.test(presentation.slug || "")
                ? presentation.slug || "null"
                : "null";
        const src = asset(avatarPortraitPath(slug, presentation.expression));
        const reducedSource = asset(
            avatarPortraitPath(
                slug,
                presentation.transition ? presentation.transition.to : presentation.expression,
            ),
        );
        setAvatarPortrait(image, src, presentation.transition, reducedMotion, reducedSource);
        image.hidden = false;
    }

    function clearNotificationCard(parts: HudNotificationCard): void {
        parts.card.hidden = true;
        parts.renderKey = "";
        parts.seal.innerHTML = "";
        parts.copy.textContent = "";
        parts.avatars.forEach(function (avatar) {
            avatar.wrapper.hidden = true;
            avatar.wrapper.className = "sow-hud__notification-avatar";
            avatar.image.hidden = true;
            avatarIdentityKeys.delete(avatar.image);
            setAvatarPortrait(avatar.image, "", null, true);
            if (avatar.image.removeAttribute) avatar.image.removeAttribute("src");
            else avatar.image.src = "";
            avatar.emblem.hidden = true;
            avatar.emblem.textContent = "";
        });
    }

    // A notice may name a hero ability by its catalog key; that name fills {ability}.
    function withAbilityName(values: Record<string, string>): Record<string, string> {
        if (!values.ability_key) return values;
        return Object.assign({}, values, { ability: SOW_t(values.ability_key, {}) });
    }

    function notificationIcon(key: string, values: Record<string, string>): string {
        // A notice that names the hero whose ability fired shows that ability's icon.
        if (/^[a-z][a-z0-9_]*$/.test(values.hero || "")) {
            const artSlug = leaderById(values.hero).art_slug || values.hero;
            return (
                "<img class='sow-hud__notification-ability' src='" +
                asset("gameplay/icons/abilities/" + artSlug + ".webp") +
                "' alt='' draggable='false'>"
            );
        }
        if (key.indexOf("nuke") >= 0) return hudIcon("nuke", "sow-hud__notification-icon");
        if (key.indexOf("betrayal") >= 0) return statusIcon("traitor", "sow-hud__notification-icon");
        if (/attack|elimination/.test(key)) return hudIcon("attack", "sow-hud__notification-icon");
        if (key.indexOf("fleet") >= 0) return hudIcon("fleet", "sow-hud__notification-icon");
        if (key.indexOf("request") >= 0) return hudIcon("inbox", "sow-hud__notification-icon");
        if (key.indexOf("alliance") >= 0) return statusIcon("allied", "sow-hud__notification-icon");
        if (key.indexOf("resource") >= 0) return hudIcon("transfer", "sow-hud__notification-icon");
        if (/build|building|gold/.test(key)) return hudIcon("tools", "sow-hud__notification-icon");
        return '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"></circle><path d="M12 7v10M7 12h10"></path></svg>';
    }

    function renderNotificationCard(
        parts: HudNotificationCard,
        item: NotificationRenderItem,
        contextual: boolean,
    ): void {
        const entry = item.entry || {};
        const key = String(entry.key || "hud.event");
        const avatars = contextual
            ? []
            : "avatars" in entry && Array.isArray(entry.avatars)
              ? entry.avatars
              : [];
        const renderKey =
            String(entry.id) +
            ":" +
            key +
            ":" +
            JSON.stringify(entry.values || {}) +
            ":" +
            JSON.stringify(avatars) +
            ":" +
            item.priority;
        parts.card.hidden = false;
        if (parts.renderKey === renderKey) return;
        parts.renderKey = renderKey;
        parts.card.className =
            "sow-hud__notification sow-hud__notification--premium" +
            (contextual ? " sow-hud__notification--contextual" : "") +
            (item.priority >= 4 ? " sow-hud__notification--urgent" : "") +
            (/resource_(received|sent)|ally_support/.test(key) ? " sow-hud__notification--support" : "");
        parts.seal.innerHTML = notificationIcon(key, entry.values || {});
        parts.avatars.forEach(function (slot, index) {
            const presentation = avatars[index] || null;
            slot.wrapper.hidden = !presentation;
            slot.wrapper.className =
                "sow-hud__notification-avatar" +
                (index > 0 && avatars[index - 1] ? " sow-hud__notification-avatar--overlap" : "");
            renderAvatarIdentity(slot.image, slot.emblem, presentation);
        });
        parts.copy.textContent = SOW_t(key, withAbilityName(entry.values || {}));
    }

    function positionMapFeedback(entry: HudMapFeedback, card: HudNotificationCard): void {
        const width = Math.max(1, window.innerWidth || 1);
        const height = Math.max(1, window.innerHeight || 1);
        const rect = card.card.getBoundingClientRect ? card.card.getBoundingClientRect() : null;
        const cardWidth = Math.min(rect && rect.width ? rect.width : 320, width - 16);
        const cardHeight = rect && rect.height ? rect.height : 52;
        let dockReserve = 0;
        if (width <= 720 && window.getComputedStyle && hudRoot) {
            dockReserve =
                Number.parseFloat(window.getComputedStyle(hudRoot).getPropertyValue("--sow-dock-reserve")) ||
                128;
        }
        let x = Number(entry.x);
        let y = Number(entry.y);
        if (!Number.isFinite(x)) x = width * 0.5;
        if (!Number.isFinite(y)) y = height * 0.5;
        const halfWidth = cardWidth * 0.5;
        const maxLeft = Math.max(halfWidth + 8, width - halfWidth - 8);
        const left = Math.max(halfWidth + 8, Math.min(maxLeft, x));
        const safeTop = Math.min(72, Math.max(8, height - cardHeight - 8));
        const maxTop = Math.max(safeTop, height - cardHeight - dockReserve - 8);
        let top = y - cardHeight - 14;
        if (top < safeTop) top = y + 14;
        top = Math.max(safeTop, Math.min(maxTop, top));
        hudRefs!.mapFeedback.style.left = left + "px";
        hudRefs!.mapFeedback.style.top = top + "px";
    }

    function measureHudScreen(): HudScreenSize {
        hudScreenSize = {
            width: (hudRoot && hudRoot.clientWidth) || window.innerWidth || 0,
            height: (hudRoot && hudRoot.clientHeight) || window.innerHeight || 0,
        };
        return hudScreenSize;
    }

    // Observers reuse the reading from the last update or viewport change instead of forcing another layout.
    function cachedHudScreen(): HudScreenSize {
        return hudScreenSize || measureHudScreen();
    }

    function compactHudDensity(width: number, height: number): number {
        width = Number(width);
        height = Number(height);
        if (
            !Number.isFinite(width) ||
            !Number.isFinite(height) ||
            width <= height ||
            width <= 0 ||
            height <= 0
        )
            return 1;
        return Math.max(0.65, Math.min(1, width / 960, height / 720));
    }

    function writeHudDensity(target: HTMLElement | null, density: number, storyRoot: boolean): void {
        if (!target) return;
        if (density < 1) {
            if (target.dataset.compact !== "true") target.dataset.compact = "true";
            const values: Record<string, string> = {
                "--sow-hud-density": density.toFixed(4),
                "--sow-hud-hit-target-zoom": (1 / density).toFixed(4),
                "--sow-hud-text-primary-size": (12 / density).toFixed(2) + "px",
                "--sow-hud-text-secondary-size": (11 / density).toFixed(2) + "px",
                "--sow-player-panel-height": (storyRoot ? 92 * density : 92).toFixed(2) + "px",
            };
            if (!storyRoot) {
                values["--sow-tutorial-progress-height"] = "24px";
                values["--sow-tutorial-progress-label-size"] = (14 / density).toFixed(2) + "px";
            }
            Object.keys(values).forEach(function (property) {
                if (target.style.getPropertyValue(property) !== values[property])
                    target.style.setProperty(property, values[property]);
            });
        } else {
            if (target.dataset.compact) delete target.dataset.compact;
            [
                "--sow-hud-density",
                "--sow-hud-hit-target-zoom",
                "--sow-hud-text-primary-size",
                "--sow-hud-text-secondary-size",
                "--sow-player-panel-height",
                "--sow-tutorial-progress-height",
                "--sow-tutorial-progress-label-size",
            ].forEach(function (property) {
                if (target.style.getPropertyValue(property)) target.style.removeProperty(property);
            });
        }
    }

    let compactHudDensityValue: number | null = null;

    function syncCompactHudDensity(hudScreen = cachedHudScreen()) {
        if (!hudRoot) return 1;
        const active = document.activeElement;
        const isEditingHudField =
            active && hudRoot.contains(active) && /^(INPUT|TEXTAREA|SELECT)$/.test(active.tagName || "");
        const density =
            isEditingHudField && compactHudDensityValue !== null
                ? compactHudDensityValue
                : compactHudDensity(hudScreen.width, hudScreen.height);
        compactHudDensityValue = density;
        writeHudDensity(hudRoot, density, false);
        writeHudDensity(document.getElementById("sow-story"), density, true);
        if (density < 1) {
            const operationCap = Math.max(0, (hudScreen.height * 0.2) / density).toFixed(1) + "px";
            if (hudRoot.style.getPropertyValue("--sow-hud-layout-operations-max-height") !== operationCap) {
                hudRoot.style.setProperty("--sow-hud-layout-operations-max-height", operationCap);
            }
        } else if (hudRoot.style.getPropertyValue("--sow-hud-layout-operations-max-height")) {
            hudRoot.style.removeProperty("--sow-hud-layout-operations-max-height");
        }
        return density;
    }

    function clearCompactHudDensity() {
        compactHudDensityValue = null;
        writeHudDensity(hudRoot, 1, false);
        writeHudDensity(document.getElementById("sow-story"), 1, true);
        if (hudRoot && hudRoot.style.getPropertyValue("--sow-hud-layout-operations-max-height")) {
            hudRoot.style.removeProperty("--sow-hud-layout-operations-max-height");
        }
        if (hudRoot && hudRoot.style.getPropertyValue("--sow-hud-layout-operations-bottom")) {
            hudRoot.style.removeProperty("--sow-hud-layout-operations-bottom");
        }
    }

    function syncDefaultDockReserve() {
        if (hudRefs!.dock && hudRefs!.dock.hidden) {
            hudRoot!.style.setProperty("--sow-dock-reserve", "0px");
        } else {
            hudRoot!.style.removeProperty("--sow-dock-reserve");
        }
    }

    function syncCompactHudBounds(hudScreen = cachedHudScreen()) {
        if (!hudRoot || !hudRefs) return;
        const viewport = window.visualViewport;
        const viewHeight = (viewport && viewport.height) || hudScreen.height;
        const viewTop = (viewport && viewport.offsetTop) || 0;
        const viewBottom = Math.max(0, hudScreen.height - viewHeight - viewTop);
        const viewportValues: Record<string, string> = {
            "--sow-hud-dialog-view-height": viewHeight + "px",
            "--sow-hud-dialog-view-top": viewTop + "px",
            "--sow-hud-dialog-view-bottom": viewBottom + "px",
        };
        Object.keys(viewportValues).forEach(function (key) {
            if (hudRoot.style.getPropertyValue(key) !== viewportValues[key])
                hudRoot.style.setProperty(key, viewportValues[key]);
        });
        const density = syncCompactHudDensity(hudScreen);
        const coarsePointer =
            window.matchMedia && window.matchMedia("(hover: none) and (pointer: coarse)").matches;
        const compact = coarsePointer && density < 1;
        // Measure before writing. The stack rects below are read after the writes they depend on.
        let operationAnchorTop = window.innerHeight;
        [hudRefs.dock].forEach(function (element) {
            if (!element || element.hidden || !element.getClientRects().length) return;
            operationAnchorTop = Math.min(operationAnchorTop, element.getBoundingClientRect().top);
        });
        let top = 0;
        let actionTop = 56;
        if (compact) {
            [
                hudRefs.topbar,
                hudRefs.nameplate,
                document.querySelector<HTMLElement>("#sow-story .sow-story__objective"),
            ].forEach(function (element) {
                if (!element || element.hidden || !element.getClientRects().length) return;
                top = Math.max(top, element.getBoundingClientRect().bottom);
            });
            if (hudRefs.topbar && hudRefs.topbar.getClientRects().length)
                actionTop = Math.ceil(hudRefs.topbar.getBoundingClientRect().bottom + 8);
        }
        if (operationAnchorTop < window.innerHeight) {
            hudRoot.style.setProperty(
                "--sow-hud-layout-operations-bottom",
                Math.ceil(window.innerHeight - operationAnchorTop + 2) + "px",
            );
        } else {
            hudRoot.style.removeProperty("--sow-hud-layout-operations-bottom");
        }
        if (!compact) {
            ["--sow-hud-layout-top-reserve", "--sow-hud-layout-bottom-edge"].forEach(function (property) {
                hudRoot.style.removeProperty(property);
            });
            syncDefaultDockReserve();
            return;
        }
        hudRoot.style.setProperty("--sow-hud-layout-top-reserve", Math.ceil(top + 8) + "px");
        const dockInner = hudRefs.dock && hudRefs.dock.querySelector<HTMLElement>(".sow-hud__dock-inner");
        const safeBottom =
            Number.parseFloat(window.getComputedStyle(hudRoot).getPropertyValue("--sow-sab")) || 0;
        let stackTop = window.innerHeight;
        let stackBottom = 0;
        [dockInner, hudRefs.operations].forEach(function (element) {
            if (!element || element.hidden || !element.getClientRects().length) return;
            const rect = element.getBoundingClientRect();
            stackTop = Math.min(stackTop, rect.top);
            stackBottom = Math.max(stackBottom, rect.bottom);
        });
        if (stackBottom === 0) stackBottom = window.innerHeight;
        const stackHeight = Math.max(0, Math.ceil(stackBottom - stackTop));
        const dockReserve = stackHeight > 0 ? stackHeight + Math.max(8, safeBottom) + 8 : 0;
        hudRoot.style.setProperty("--sow-dock-reserve", dockReserve + "px");
        hudRoot.style.setProperty("--sow-hud-layout-bottom-edge", Math.floor(stackTop - 8) + "px");
        const freeOperationsHeight = Math.max(
            0,
            operationAnchorTop - Math.max(actionTop, top + 8) - safeBottom - 6,
        );
        const operationCap =
            Math.max(0, Math.min(window.innerHeight * 0.2, freeOperationsHeight) / density).toFixed(1) + "px";
        if (hudRoot.style.getPropertyValue("--sow-hud-layout-operations-max-height") !== operationCap) {
            hudRoot.style.setProperty("--sow-hud-layout-operations-max-height", operationCap);
        }
        if (activeMapFeedback && mapFeedbackCard && hudRefs.mapFeedback && !hudRefs.mapFeedback.hidden)
            positionMapFeedback(activeMapFeedback.entry, mapFeedbackCard);
    }

    function clearMapFeedback() {
        if (mapFeedbackTimer) window.clearTimeout(mapFeedbackTimer);
        mapFeedbackTimer = null;
        activeMapFeedback = null;
        if (hudRefs && hudRefs.mapFeedback) {
            hudRefs.mapFeedback.hidden = true;
            hudRefs.mapFeedback.style.removeProperty("left");
            hudRefs.mapFeedback.style.removeProperty("top");
        }
        if (mapFeedbackCard) {
            mapFeedbackCard.card.classList.remove("sow-hud__map-feedback-card--visible");
            clearNotificationCard(mapFeedbackCard);
        }
    }

    function renderMapFeedback(entry: HudMapFeedback | null): void {
        if (!hudRefs || !hudRefs.mapFeedback || !mapFeedbackCard) return;
        const card = mapFeedbackCard;
        if (!entry) {
            if (activeMapFeedback || mapFeedbackTimer) clearMapFeedback();
            return;
        }
        const id = Number(entry.id);
        if (!Number.isSafeInteger(id) || id <= mapFeedbackCursor) return;
        mapFeedbackCursor = id;
        const age = Math.max(0, Number(entry.age_ms) || 0);
        clearMapFeedback();
        if (age >= MAP_FEEDBACK_DURATION_MS) return;
        const now = Date.now();
        const feedback: ActiveMapFeedback = {
            entry: entry,
            priority: 1,
            expiresAt: now + MAP_FEEDBACK_DURATION_MS - age,
        };
        activeMapFeedback = feedback;
        hudRefs.mapFeedback.hidden = false;
        renderNotificationCard(card, feedback, true);
        positionMapFeedback(entry, card);
        card.card.classList.remove("sow-hud__map-feedback-card--visible");
        void card.card.offsetWidth;
        card.card.classList.add("sow-hud__map-feedback-card--visible");
        mapFeedbackTimer = window.setTimeout(
            function () {
                card.card.classList.remove("sow-hud__map-feedback-card--visible");
                mapFeedbackTimer = window.setTimeout(function () {
                    mapFeedbackTimer = null;
                    clearMapFeedback();
                }, MAP_FEEDBACK_FADE_OUT_MS);
            },
            Math.max(0, feedback.expiresAt - Date.now()),
        );
    }

    function createPlayerIdentity(): PlayerIdentityParts {
        const identity = document.createElement("div");
        identity.className = "sow-hud__request-identity";
        const avatar = document.createElement("span");
        avatar.className = "sow-hud__notification-avatar";
        const portrait = document.createElement("img");
        portrait.className = "sow-hud__notification-portrait";
        portrait.alt = "";
        portrait.draggable = false;
        bindAvatarFallback(portrait);
        const emblem = document.createElement("span");
        emblem.className = "sow-hud__notification-emblem";
        avatar.append(portrait, emblem);
        const copy = document.createElement("div");
        copy.className = "sow-hud__request-copy";
        const name = document.createElement("bdi");
        name.className = "sow-hud__request-name";
        copy.appendChild(name);
        identity.append(avatar, copy);
        return { element: identity, portrait: portrait, emblem: emblem, copy: copy, name: name };
    }

    function renderPlayerIdentity(
        parts: PlayerIdentityParts,
        name: string | null | undefined,
        avatar: HudAvatarIdentity | null | undefined,
    ): void {
        const displayName = name || SOW_t("hud.player_name");
        if (parts.name.textContent !== displayName) parts.name.textContent = displayName;
        renderAvatarIdentity(parts.portrait, parts.emblem, avatar || { kind: "fallback" });
    }

    function restoreHudFocus(target: HTMLElement | null): void {
        if (target && !target.isConnected && target.id) target = document.getElementById(target.id);
        if (!target || !target.isConnected || !target.getClientRects().length)
            target = document.getElementById("blade");
        if (target && typeof target.focus === "function") target.focus({ preventScroll: true });
    }

    function createInboxRequest(request: HudInboxRequest): InboxRequestParts {
        const element = document.createElement("article");
        element.className = "sow-hud__request-card";
        const identity = createPlayerIdentity();
        const type = document.createElement("span");
        type.className = "sow-hud__request-type";
        const resources = document.createElement("div");
        resources.className = "sow-hud__request-resources";
        const amounts: InboxRequestParts["amounts"] = {
            gold: { element: document.createElement("span"), value: document.createElement("bdi") },
            troops: { element: document.createElement("span"), value: document.createElement("bdi") },
        };
        (["gold", "troops"] as const).forEach(function (kind) {
            const metric = amounts[kind].element;
            metric.className = "sow-hud__request-metric";
            if (kind === "gold") {
                const icon = document.createElement("img");
                icon.className = "sow-hud__currency-icon sow-hud__currency-icon--gold";
                icon.src = currencyAsset("gold");
                icon.alt = "";
                metric.appendChild(icon);
            } else metric.innerHTML = hudIcon("troops", "sow-hud__inline-icon");
            const value = amounts[kind].value;
            metric.appendChild(value);
            resources.appendChild(metric);
            amounts[kind] = { element: metric, value: value };
        });
        identity.copy.append(type, resources);
        const actions = document.createElement("div");
        actions.className = "sow-hud__request-actions";
        const buttons = (["accept", "reject"] as const).map(function (action) {
            const button = document.createElement("button");
            button.type = "button";
            button.className = "sow-hud__request-" + action;
            button.dataset.command =
                action + (request.kind === "resources" ? "_resource_request" : "_alliance");
            button.dataset.playerId = String(request.requester_id);
            actions.appendChild(button);
            return button;
        });
        element.append(identity.element, actions);
        return {
            element: element,
            portrait: identity.portrait,
            emblem: identity.emblem,
            copy: identity.copy,
            name: identity.name,
            type: type,
            resources: resources,
            amounts: amounts,
            accept: buttons[0],
            reject: buttons[1],
        };
    }

    function renderInbox(requests: HudInboxRequest[]): void {
        if (!hudRefs || !hudRefs.inboxRows || !Array.isArray(requests)) return;
        const renderKey = String(window.SOW_LOCALE || "en") + ":" + JSON.stringify(requests);
        if (renderKey === hudRefs.inboxRenderKey) return;
        const previous = hudRefs.inboxRequests;
        const previousKeys = hudRefs.inboxRequestKeys;
        const focusedKey = previousKeys.find(function (key) {
            return previous[key].element.contains(document.activeElement);
        });
        const next: Record<string, InboxRequestParts> = Object.create(null);
        const keys: string[] = [];
        const numbers = new Intl.NumberFormat(window.SOW_LOCALE || "en", { maximumFractionDigits: 0 });
        requests.forEach(function (request) {
            if (
                !request ||
                (request.kind !== "alliance" && request.kind !== "resources") ||
                !Number.isSafeInteger(request.requester_id) ||
                request.requester_id <= 0
            )
                return;
            const key = request.kind + ":" + request.requester_id;
            if (next[key]) return;
            const row = previous[key] || createInboxRequest(request);
            const name = request.name || SOW_t("hud.player_name");
            renderPlayerIdentity(row, name, request.avatar);
            row.type.textContent = SOW_t(
                request.kind === "resources" ? "hud.resource_request_title" : "hud.alliance_request_title",
            );
            const values: Record<string, string> = { name: name };
            (["gold", "troops"] as const).forEach(function (kind) {
                let value = Number(request[kind]);
                value = Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0;
                values[kind] = numbers.format(value);
                row.amounts[kind].value.textContent = values[kind];
                row.amounts[kind].element.hidden = value === 0;
                row.amounts[kind].element.setAttribute(
                    "aria-label",
                    SOW_t(kind === "gold" ? "hud.gold" : "hud.troops") + ": " + values[kind],
                );
            });
            row.resources.hidden = request.kind !== "resources";
            row.element.setAttribute(
                "aria-label",
                SOW_t(request.kind === "resources" ? "hud.resource_request" : "hud.alliance_request", values),
            );
            row.accept.textContent = SOW_t("hud.accept");
            row.reject.textContent = SOW_t("hud.reject");
            row.accept.disabled = request.active === false;
            row.accept.title = row.accept.disabled ? SOW_t("hud.action_unavailable") : "";
            row.element.classList.toggle("is-inactive", row.accept.disabled);
            next[key] = row;
            keys.push(key);
        });
        previousKeys.forEach(function (key) {
            if (!next[key]) previous[key].element.remove();
        });
        keys.forEach(function (key, index) {
            const row = next[key].element;
            const before = hudRefs!.inboxRows!.children[index] || null;
            if (before !== row) hudRefs!.inboxRows.insertBefore(row, before);
        });
        hudRefs.inboxRequests = next;
        hudRefs.inboxRequestKeys = keys;
        hudRefs.inboxRenderKey = renderKey;
        hudRefs.inboxRows.hidden = keys.length === 0;
        hudRefs.inboxEmpty!.hidden = keys.length !== 0;
        hudRefs.inboxTotal!.textContent = numbers.format(keys.length);
        if (focusedKey && !next[focusedKey]) {
            const nextRow = next[keys[Math.min(previousKeys.indexOf(focusedKey), keys.length - 1)]];
            (nextRow
                ? nextRow.accept.disabled
                    ? nextRow.reject
                    : nextRow.accept
                : hudRefs.inboxClose
            ).focus();
        }
    }

    function renderNotifications(
        entries: HudNotification[],
        forceRefresh: boolean,
        hudScreen: HudScreenSize = cachedHudScreen(),
    ): void {
        if (!hudRefs || !hudRefs.notifications || !Array.isArray(entries)) return;
        let changed = Boolean(forceRefresh);
        const limit = window.SOWCampaignView.isCompactViewport(hudRoot, hudScreen.width, hudScreen.height)
            ? 1
            : 3;
        const now = Date.now();
        const unexpired = activeNotifications.filter(function (item) {
            return item.expiresAt > now;
        });
        if (unexpired.length !== activeNotifications.length) changed = true;
        activeNotifications = unexpired;
        if (activeNotifications.length > limit) {
            activeNotifications.sort(function (a, b) {
                return b.priority - a.priority || b.createdAt - a.createdAt;
            });
            activeNotifications.length = limit;
            changed = true;
        }
        entries.forEach(function (entry) {
            const id = Number(entry && entry.id);
            if (!Number.isSafeInteger(id) || id <= notificationCursor) return;
            notificationCursor = id;
            const age = Math.max(0, Number(entry.age_ms) || 0);
            if (age >= 3600) return;
            const createdAt = Date.now();
            const item = {
                entry: entry,
                priority: Math.max(0, Number(entry.priority) || 0),
                group: entry.group ? String(entry.group) : "event:" + id,
                sumValues: Boolean(entry.sum_values),
                expiresAt: createdAt + 3600 - age,
                createdAt: createdAt,
                totals: null,
            };
            const existing = activeNotifications.find(function (candidate) {
                return candidate.group === item.group;
            });
            if (existing) {
                if (item.sumValues) {
                    const merged = Object.assign({}, existing.entry.values || {});
                    const totals = existing.totals || Object.create(null);
                    ["gold", "troops", "count", "attackers"].forEach(function (key) {
                        const addedRaw = (entry.values || {})[key];
                        if (totals[key] == null && merged[key] != null) {
                            totals[key] = Number(String(merged[key]).replace(/,/g, "")) || 0;
                        }
                        if (addedRaw != null) {
                            const added = Number(String(addedRaw).replace(/,/g, ""));
                            if (Number.isFinite(added)) totals[key] = (totals[key] || 0) + added;
                        }
                        if (totals[key] != null) {
                            merged[key] = new Intl.NumberFormat(window.SOW_LOCALE || "en", {
                                maximumFractionDigits: 0,
                            }).format(totals[key]);
                        }
                    });
                    let mergedKey = entry.key;
                    if (item.group.indexOf("resource:") === 0) {
                        const direction = item.group.split(":")[1];
                        const hasGold = Number(totals.gold || 0) > 0;
                        const hasTroops = Number(totals.troops || 0) > 0;
                        const resourceSuffix = hasGold && hasTroops ? "both" : hasGold ? "gold" : "troops";
                        mergedKey = "hud.resource_" + direction + "_" + resourceSuffix;
                    }
                    item.entry = Object.assign({}, entry, { key: mergedKey, values: merged });
                    item.totals = totals;
                }
                existing.entry = item.entry;
                existing.priority = Math.max(existing.priority, item.priority);
                existing.expiresAt = item.expiresAt;
                existing.createdAt = item.createdAt;
                existing.sumValues = item.sumValues;
                existing.group = item.group;
                existing.totals = item.totals;
                changed = true;
                return;
            }
            if (activeNotifications.length < limit) {
                activeNotifications.push(item);
                changed = true;
                return;
            }
            let replaceIndex = 0;
            for (let i = 1; i < activeNotifications.length; i += 1) {
                const current = activeNotifications[i];
                const candidate = activeNotifications[replaceIndex];
                if (
                    current.priority < candidate.priority ||
                    (current.priority === candidate.priority && current.createdAt < candidate.createdAt)
                )
                    replaceIndex = i;
            }
            if (item.priority >= activeNotifications[replaceIndex].priority) {
                activeNotifications[replaceIndex] = item;
                changed = true;
            }
        });
        hudRoot!.dataset.notificationsVisible = String(activeNotifications.length > 0);
        if (!changed) return;
        activeNotifications.sort(function (a, b) {
            return b.priority - a.priority || a.createdAt - b.createdAt;
        });
        notificationCards.forEach(function (parts, index) {
            const item = activeNotifications[index];
            if (!item) {
                clearNotificationCard(parts);
                return;
            }
            renderNotificationCard(parts, item, false);
        });
        if (notificationTimer) window.clearTimeout(notificationTimer);
        notificationTimer = null;
        if (activeNotifications.length) {
            const nextExpiry = Math.min(
                ...activeNotifications.map(function (item) {
                    return item.expiresAt;
                }),
            );
            notificationTimer = window.setTimeout(
                function () {
                    notificationTimer = null;
                    renderNotifications([], true);
                },
                Math.max(0, nextExpiry - Date.now()),
            );
        }
    }

    function createCombatOperationRow(): CombatOperationRow {
        const element = document.createElement("article");
        const portrait = document.createElement("img");
        const glyph = document.createElement("span");
        const avatar = document.createElement("span");
        const copy = document.createElement("span");
        const name = document.createElement("b");
        const details = document.createElement("small");
        const detailsLabel = document.createElement("span");
        const detailsSeparator = document.createElement("span");
        const detailsEta = document.createElement("span");
        const troops = document.createElement("b");
        const actions = document.createElement("span");
        const emblem = document.createElement("span");
        element.className = "sow-hud__operation";
        avatar.className = "sow-hud__operation-avatar";
        portrait.className = "sow-hud__operation-portrait";
        portrait.alt = "";
        portrait.draggable = false;
        portrait.loading = "lazy";
        bindAvatarFallback(portrait);
        emblem.className = "sow-hud__operation-emblem sow-hud__notification-emblem";
        emblem.hidden = true;
        glyph.className = "sow-hud__operation-glyph";
        copy.className = "sow-hud__operation-copy";
        name.className = "sow-hud__operation-name";
        details.className = "sow-hud__operation-details";
        detailsLabel.className = "sow-hud__operation-details-label";
        detailsSeparator.className = "sow-hud__operation-details-separator";
        detailsSeparator.hidden = true;
        detailsEta.className = "sow-hud__operation-details-eta";
        detailsEta.hidden = true;
        troops.className = "sow-hud__operation-troops";
        actions.className = "sow-hud__operation-actions";
        avatar.append(portrait, emblem, glyph);
        details.append(detailsLabel, detailsSeparator, detailsEta);
        copy.append(name, details);
        element.append(avatar, copy, troops, actions);
        return {
            element: element,
            portrait: portrait,
            emblem: emblem,
            glyph: glyph,
            name: name,
            details: details,
            detailsLabel: detailsLabel,
            detailsSeparator: detailsSeparator,
            detailsEta: detailsEta,
            troops: troops,
            actions: actions,
            actionKey: "",
        };
    }

    function operationAction(
        command: string,
        glyph: string,
        label: string,
        operation: HudCombatOperation,
    ): HTMLButtonElement {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "sow-hud__operation-action";
        button.dataset.command = command;
        button.setAttribute("aria-label", label);
        button.title = label;
        button.innerHTML =
            command === "scuttle_fleet"
                ? statusIcon("death", "sow-hud__operation-action-icon")
                : hudIcon(glyph, "sow-hud__operation-action-icon");
        if (command === "focus_world") {
            button.dataset.focusX = String(operation.focus_x);
            button.dataset.focusY = String(operation.focus_y);
        } else if (command === "counter_attack") {
            button.dataset.playerId = String(operation.player_id);
            button.dataset.incomingTroops = String(operation.incoming_troops || operation.troops || 0);
        } else if (command === "cancel_attack") {
            button.dataset.attackId = String(operation.id);
        } else if (command === "recall_fleet") {
            button.dataset.fleetId = String(operation.id);
        } else if (command === "scuttle_fleet") {
            button.dataset.fleetId = String(operation.id);
        }
        return button;
    }

    function renderCombatOperations(operations: HudCombatOperation[]): void {
        if (!hudRefs || !hudRefs.operations || !hudRefs.operationRows) return;
        operations = Array.isArray(operations) ? operations : [];
        const incomingTotals: Record<string, number> = Object.create(null);
        operations.forEach(function (operation) {
            if (operation.direction === "incoming" && !operation.retreating) {
                const playerId = String(operation.player_id);
                incomingTotals[playerId] =
                    (incomingTotals[playerId] || 0) + Math.max(0, Number(operation.troops) || 0);
            }
        });
        Object.keys(pendingCounterAttacks).forEach(function (playerId) {
            if ((incomingTotals[playerId] || 0) < pendingCounterAttacks[playerId] - 0.5) {
                delete pendingCounterAttacks[playerId];
            }
        });

        const nextRows: Record<string, CombatOperationRow> = Object.create(null);
        const nextKeys: string[] = [];
        operations.forEach(function (operation) {
            if (!operation || (operation.kind !== "attack" && operation.kind !== "fleet")) return;
            const key = operation.kind + ":" + operation.id;
            nextKeys.push(key);
            const row = hudRefs!.combatOperationRows[key] || createCombatOperationRow();
            const incoming = operation.direction === "incoming";
            const outgoing = operation.direction === "outgoing";
            const neutral = Boolean(operation.neutral);
            const retreating = Boolean(operation.retreating);
            const playerId = String(operation.player_id);
            const name = neutral ? SOW_t("hud.land") : operation.name || SOW_t("hud.player_name");
            const troops = Math.max(0, Number(operation.troops) || 0);
            let details =
                operation.kind === "attack" && incoming
                    ? SOW_t("hud.attack_incoming", { count: formatNameplateTroops(troops) })
                    : SOW_t(operation.kind === "fleet" ? "hud.map_action_fleet" : "hud.map_action_attack");
            let detailsEta = "";
            if (operation.kind === "fleet" && outgoing) {
                details = SOW_t(retreating ? "hud.returning_transport" : "hud.map_action_fleet");
                const etaSeconds = Number(operation.eta_seconds);
                if (operation.eta_seconds != null && Number.isFinite(etaSeconds)) {
                    detailsEta = SOW_t("hud.transport_eta", {
                        seconds: String(Math.max(1, Math.ceil(etaSeconds))),
                    });
                }
            }
            row.element.classList.toggle("is-incoming", incoming);
            row.element.classList.toggle("is-outgoing", outgoing);
            row.element.classList.toggle("is-retreating", retreating);
            row.element.classList.toggle("is-neutral", neutral);
            renderAvatarIdentity(
                row.portrait,
                row.emblem,
                operation.avatar && typeof operation.avatar === "object"
                    ? operation.avatar
                    : { kind: "fallback" },
            );
            if (row.name.textContent !== name) row.name.textContent = name;
            if (row.detailsLabel.textContent !== details) row.detailsLabel.textContent = details;
            const detailsSeparatorText = detailsEta === "" ? "" : " · ";
            if (row.detailsSeparator.textContent !== detailsSeparatorText)
                row.detailsSeparator.textContent = detailsSeparatorText;
            row.detailsSeparator.hidden = detailsEta === "";
            if (row.detailsEta.textContent !== detailsEta) row.detailsEta.textContent = detailsEta;
            row.detailsEta.hidden = detailsEta === "";
            const troopText = formatNameplateTroops(troops);
            if (row.troops.textContent !== troopText) row.troops.textContent = troopText;
            const canFocus =
                operation.focus_x != null &&
                operation.focus_y != null &&
                Number.isFinite(Number(operation.focus_x)) &&
                Number.isFinite(Number(operation.focus_y));
            const canCancel = outgoing && operation.kind === "attack" && !retreating;
            const canCounter = incoming && !retreating;
            const canRecall = outgoing && operation.kind === "fleet" && !retreating;
            const canScuttle = outgoing && operation.kind === "fleet";
            const pending = canCounter && pendingCounterAttacks[playerId] != null;
            const actionKey = [canFocus, canCancel, canCounter, canRecall, canScuttle, pending].join(":");
            if (row.actionKey !== actionKey) {
                row.actions.replaceChildren();
                if (canFocus)
                    row.actions.appendChild(
                        operationAction("focus_world", "camera", SOW_t("hud.center_camera"), operation),
                    );
                if (canCounter) {
                    const counterOperation = Object.assign({}, operation, {
                        incoming_troops: incomingTotals[playerId],
                    });
                    const counterButton = operationAction(
                        "counter_attack",
                        "attack",
                        SOW_t("hud.map_action_attack"),
                        counterOperation,
                    );
                    counterButton.disabled = pending;
                    row.actions.appendChild(counterButton);
                }
                if (canCancel) {
                    const cancelLabel = SOW_t("endgame.cancel");
                    row.actions.appendChild(
                        operationAction("cancel_attack", "minus", cancelLabel, operation),
                    );
                }
                if (canRecall) {
                    row.actions.appendChild(
                        operationAction("recall_fleet", "transfer", SOW_t("hud.recall_transport"), operation),
                    );
                }
                if (canScuttle) {
                    row.actions.appendChild(
                        operationAction("scuttle_fleet", "death", SOW_t("hud.destroy_transport"), operation),
                    );
                }
                row.actionKey = actionKey;
            }
            row.actions
                .querySelectorAll<HTMLButtonElement>('[data-command="focus_world"]')
                .forEach(function (button) {
                    button.dataset.focusX = String(operation.focus_x);
                    button.dataset.focusY = String(operation.focus_y);
                });
            row.actions
                .querySelectorAll<HTMLButtonElement>('[data-command="counter_attack"]')
                .forEach(function (button) {
                    button.dataset.incomingTroops = String(incomingTotals[playerId] || 0);
                });
            nextRows[key] = row;
        });

        const orderKey = nextKeys.join("\u001e");
        if (orderKey !== hudRefs.combatOperationsOrderKey) {
            const fragment = document.createDocumentFragment();
            nextKeys.forEach(function (key) {
                fragment.appendChild(nextRows[key].element);
            });
            hudRefs.operationRows.replaceChildren(fragment);
            hudRefs.combatOperationsOrderKey = orderKey;
        }
        hudRefs.combatOperationRows = nextRows;
        hudRefs.operations.hidden = nextKeys.length === 0;
    }

    function renderHoverCard(hov: NonNullable<MenuHudPayload["hovered"]> | null): void {
        if (!hudRefs!.hoverCard) return;
        if (hov) {
            hudRefs!.hoverCard.classList.remove("hidden");
            if (hudRefs!.hoverName) hudRefs!.hoverName.textContent = hov.name || SOW_t("hud.territory");
            if (hudRefs!.hoverPct)
                hudRefs!.hoverPct.textContent = Math.round((hov.territory_pct || 0) * 100) + "%";
            if (hudRefs!.hoverTroops)
                hudRefs!.hoverTroops.textContent =
                    hov.troops > 1000
                        ? (hov.troops / 1000).toFixed(1) + "k"
                        : String(Math.floor(hov.troops || 0));
            if (hudRefs!.hoverGold)
                hudRefs!.hoverGold.textContent = Math.floor(hov.gold || 0).toLocaleString();
            if (hudRefs!.hoverBlds) {
                const bldText = [];
                if (hov.cities > 0)
                    bldText.push(buildingIcon("City", 1, "sow-hud__inline-icon") + " x" + hov.cities);
                if (hov.factories > 0)
                    bldText.push(buildingIcon("Factory", 1, "sow-hud__inline-icon") + " x" + hov.factories);
                if (hov.ports > 0)
                    bldText.push(buildingIcon("Port", 1, "sow-hud__inline-icon") + " x" + hov.ports);
                if (hov.towers > 0)
                    bldText.push(buildingIcon("Tower", 1, "sow-hud__inline-icon") + " x" + hov.towers);
                if (hov.farms > 0)
                    bldText.push(buildingIcon("Farm", 1, "sow-hud__inline-icon") + " x" + hov.farms);
                hudRefs!.hoverBlds.innerHTML = bldText.join(" ");
            }
        } else {
            hudRefs!.hoverCard.classList.add("hidden");
        }
    }

    function renderHud(forceNotifications = false): void {
        if (!hudRoot) return;
        if (!hudState || hudState.phase !== "Playing" || !hudState.hud) {
            hudRoot.hidden = true;
            if (hudDevLayer) hudDevLayer.hidden = true;
            clearCompactHudDensity();
            activeHudPanel = null;
            betrayalOpen = false;
            emoteTextKey = "";
            devSidebarOpen = false;

            hudRoot.dataset.overlayOpen = "false";
            hudRoot.dataset.notificationsVisible = "false";
            if (hudRefs && hudRefs.mapMenu) hudRefs.mapMenu.classList.add("hidden");
            leaderboardRows = Object.create(null);
            leaderboardRenderKey = "";
            lastLeaderboardPlayers = [];
            lastLeaderboardTeams = [];
            if (hudRefs) {
                hudRefs.inboxRequests = Object.create(null);
                hudRefs.inboxRequestKeys = [];
                hudRefs.inboxRenderKey = "";
                if (hudRefs.inboxRows) hudRefs.inboxRows.replaceChildren();
            }
            if (hudRefs && hudRefs.rows) hudRefs.rows.replaceChildren();
            if (notificationTimer) window.clearTimeout(notificationTimer);
            notificationTimer = null;
            activeNotifications = [];
            renderCombatOperations([]);
            pendingCounterAttacks = Object.create(null);
            notificationCards.forEach(clearNotificationCard);
            notificationCursor = 0;
            mapFeedbackCursor = 0;
            clearMapFeedback();
            return;
        }
        ensureHudDom();
        hudRoot.hidden = false;
        const hudScreen = measureHudScreen();
        syncCompactHudDensity(hudScreen);

        const hud = hudState.hud;
        const settings = hudState.settings || DEFAULT_HUD_SETTINGS;
        const settingsOpen = activeHudPanel === "settings";
        const inboxOpen = activeHudPanel === "inbox";
        const leaderboardOpen = activeHudPanel === "leaderboard";
        hudRoot.classList.toggle(
            "is-reduced-motion",
            Boolean(hudState.settings && hudState.settings.reduced_motion),
        );
        const devTools = hud.dev_tools;
        const showDevTools = settings.show_dev_tools_available && settings.show_dev_tools;
        const diagnosticsVisible = settings.show_fps_ping === true;
        if (hudRefs!.diagnostics) hudRefs!.diagnostics.hidden = !diagnosticsVisible;
        if (hudRefs!.devBtn) {
            hudRefs!.devBtn.classList.toggle("hidden", !showDevTools);
            hudRefs!.devBtn.hidden = !showDevTools;
        }
        if (hudDevLayer) hudDevLayer.hidden = !showDevTools && !devSidebarOpen;
        if (devTools.schema && hudRefs!.devSidebar) renderDevControls(devTools);
        const devConfig = devTools.config || {};
        if (hudRefs!.devSidebar) {
            hudRefs!.devSidebar.querySelectorAll<HTMLInputElement>("[data-dev]").forEach(function (input) {
                if (document.activeElement === input) return;
                const field = input.dataset.dev;
                if (!field) return;
                const value = devConfig[field];
                if (value == null) return;
                const control = input.closest<HTMLElement>("[data-dev-control]");
                if (!control) return;
                const decimals = Number(control.dataset.devDecimals) || 0;
                if (control.dataset.devKind === "bool") input.checked = Boolean(value);
                else if (input.type === "radio") input.checked = Number(input.value) === Number(value);
                else if (control.dataset.devKind === "color" || control.dataset.devKind === "vector") {
                    if (input.hasAttribute("data-dev-channel") && Array.isArray(value)) {
                        const part = value[Number(input.dataset.devChannel)];
                        if (part === undefined) return;
                        input.value = input.type === "range" ? String(part) : devFormat(part, decimals);
                        devReadout(input, decimals);
                    }
                } else {
                    const numericValue = typeof value === "number" ? value : Number(value);
                    input.value =
                        input.type === "range" ? String(numericValue) : devFormat(numericValue, decimals);
                }
            });
            hudRefs!.devSidebar
                .querySelectorAll<HTMLInputElement>("[data-dev-picker]")
                .forEach(function (picker) {
                    const field = picker.dataset.devPicker;
                    if (!field) return;
                    const value = devConfig[field];
                    if (Array.isArray(value) && document.activeElement !== picker)
                        picker.value = devColorHex(value, picker.dataset.devLinear === "1");
                });
        }
        const gold = Math.floor(hud.gold || 0);
        const goldRate = Number(hud.gold_rate) || 0;
        const troops = Math.floor(hud.troops || 0);
        const maxTroops = Math.floor(hud.max_troops || 0);
        const prod = Math.floor(hud.troop_rate || 0);
        const currentRatio = Number.isFinite(hud.attack_ratio) ? hud.attack_ratio : 0.5;
        const spawnSecs = hud.spawn_timer_secs;
        const isDeploying = spawnSecs != null && spawnSecs > 0;
        let hasBuildingItems = false;
        let hasFleetItems = false;

        const goldText = formatHudMetric(gold);
        if (hudRefs!.plateGold && hudRefs!.plateGold.dataset.val !== goldText) {
            hudRefs!.plateGold.textContent = goldText;
            hudRefs!.plateGold.dataset.val = goldText;
        }
        const goldRateText = formatHudRate(goldRate);
        if (hudRefs!.plateGoldRate && hudRefs!.plateGoldRate.textContent !== goldRateText) {
            hudRefs!.plateGoldRate.textContent = goldRateText;
        }
        if (hudRefs!.plateGoldRate) hudRefs!.plateGoldRate.classList.toggle("is-negative", goldRate < 0);
        if (hudRefs!.plateGoldWrap) hudRefs!.plateGoldWrap.classList.toggle("is-in-debt", gold < 0);
        const troopRateText = formatHudRate(prod);
        if (hudRefs!.plateTroopRate && hudRefs!.plateTroopRate.textContent !== troopRateText) {
            hudRefs!.plateTroopRate.textContent = troopRateText;
        }

        const playerPlateHidden = Boolean(hud.is_spectating);
        if (hudRefs!.nameplate) {
            if (hudRefs!.nameplate.hidden !== playerPlateHidden)
                hudRefs!.nameplate.hidden = playerPlateHidden;
            if (!playerPlateHidden) {
                const plateLeader = leaderById(hud.player_leader || (hudState && hudState.selected_leader));
                const plateSrc = asset(
                    avatarPortraitPath(
                        plateLeader.art_slug || plateLeader.slug,
                        hud.player_avatar_expression,
                    ),
                );
                const platePlayerName = String(hud.player_name || "");
                if (hudRefs!.plateAvatar)
                    setAvatarPortrait(
                        hudRefs!.plateAvatar,
                        plateSrc,
                        hud.player_avatar_transition,
                        Boolean(hudState.settings && hudState.settings.reduced_motion),
                        asset(
                            avatarPortraitPath(
                                plateLeader.art_slug || plateLeader.slug,
                                hud.player_avatar_transition
                                    ? hud.player_avatar_transition.to
                                    : hud.player_avatar_expression,
                            ),
                        ),
                    );
                if (hudRefs!.plateName && hudRefs!.plateName.textContent !== platePlayerName) {
                    hudRefs!.plateName.textContent = platePlayerName;
                }
                const plateText = formatNameplateTroops(troops) + " / " + formatNameplateTroops(maxTroops);
                if (hudRefs!.plateTroops && hudRefs!.plateTroops.textContent !== plateText) {
                    hudRefs!.plateTroops.textContent = plateText;
                }
                const plateFillPct =
                    maxTroops > 0 ? Math.min(100, Math.max(0, (troops / maxTroops) * 100)) : 0;
                const plateFillStr = plateFillPct.toFixed(1) + "%";
                if (hudRefs!.plateFill && hudRefs!.plateFill.style.width !== plateFillStr) {
                    hudRefs!.plateFill.style.width = plateFillStr;
                }
            }
        }

        if (hudRefs!.fps) {
            hudRefs!.fps.hidden = settings.show_fps_ping !== true;
            const fps = hud.fps;
            const hasFps = fps !== null && Number.isFinite(fps) && fps > 0;
            hudRefs!.fps.textContent = SOW_t("hud.fps", { fps: hasFps ? fps : "--" });
        }
        if (hudRefs!.ping) {
            const ping = hud.ping;
            const hasPing = settings.show_fps_ping === true && ping !== null && Number.isFinite(ping);
            hudRefs!.ping.hidden = !hasPing;
            if (hasPing && ping !== null) hudRefs!.ping.textContent = Math.floor(ping) + "ms";
        }
        if (hudRefs!.inboxCount) {
            hudRefs!.inboxCount.textContent = String(hud.inbox_count || 0);
            hudRefs!.inboxCount.hidden = !hud.inbox_count;
        }

        // Hover Card
        renderHoverCard(hud.hovered || null);

        // Left Rail Slider
        if (hudRefs!.slider && document.activeElement !== hudRefs!.slider) {
            hudRefs!.slider.value = String(Math.round(currentRatio * 100));
        }
        const displayRatio =
            hudRefs!.slider && document.activeElement === hudRefs!.slider
                ? Number(hudRefs!.slider.value) / 100
                : currentRatio;
        updateArmyAllocation(hud.troops, displayRatio);

        // Bottom Dock: Phase Transformation
        if (hudRefs!.deployBtn) {
            hudRefs!.deployBtn.style.display = isDeploying ? "flex" : "none";
            if (isDeploying && hudRefs!.deployTimer) {
                hudRefs!.deployTimer.textContent = spawnSecs.toFixed(1) + "s";
            }
        }
        if (hudRefs!.buildingsStrip) {
            const selectedBuilding = hud.selected_building;
            const buildingCosts = hud.building_costs || {};
            const buildUnlocks = campaignUnlocks(hud);
            hudRefs!.buildingButtons.forEach(function (button) {
                const kind = button.dataset.kind || "";
                const available = campaignAllowsBuilding(buildUnlocks, kind, 1);
                button.hidden = !available;
                button.style.display = available ? "" : "none";
                const cost = Number(buildingCosts[kind.toLowerCase()]);
                setTileCost(button, cost);
                const hasCost = Number.isFinite(cost) && cost > 0;
                const affordable = !hasCost || gold >= cost;
                const selected = selectedBuilding === kind;
                button.disabled = false;
                const disabled = !affordable && !selected;
                button.setAttribute("aria-disabled", "false");
                button.classList.toggle("is-unaffordable", disabled);
                button.classList.toggle("active", selected);
                button.setAttribute("aria-pressed", String(selected));
                const costText = hasCost ? Math.floor(cost).toLocaleString() + "g" : "";
                const label = button.dataset.label || button.getAttribute("aria-label") || kind;
                button.dataset.label = label;
                const reason = mapActionReason({ disabled: !affordable, cost: cost }, gold);
                const accessibleLabel = reason
                    ? label + ". " + reason
                    : costText
                      ? label + " " + costText
                      : label;
                button.setAttribute("aria-label", accessibleLabel);
                if (button.title !== accessibleLabel) button.title = accessibleLabel;
            });
            hasBuildingItems = hudRefs!.buildingButtons.some(function (button) {
                return !button.hidden;
            });
            hudRefs!.buildingsStrip.style.display = !isDeploying && hasBuildingItems ? "flex" : "none";
        }
        if (hudRefs!.fleetStrip) {
            const fleetPanel = hud.fleet_panel || {};
            const fleetUnlocks = campaignUnlocks(hud);
            const portLevels = Math.max(0, Number(fleetPanel.port_levels) || 0);
            const cityLevel = Math.max(0, Number(fleetPanel.city_level) || 0);
            const tradeRequiredPortLevel = Math.max(1, Number(fleetPanel.trade_required_port_level) || 1);
            const warshipRequiredPortLevel = Math.max(1, Number(fleetPanel.warship_required_port_level) || 1);
            const nukeRequiredCityLevel = Math.max(1, Number(fleetPanel.nuke_required_city_level) || 1);
            const tradeCampaignLocked =
                !campaignAllowsAction(fleetUnlocks, "trade_ship") ||
                !campaignAllowsBuilding(fleetUnlocks, "Port", tradeRequiredPortLevel);
            const warshipCampaignLocked =
                !campaignAllowsAction(fleetUnlocks, "warship") ||
                !campaignAllowsBuilding(fleetUnlocks, "Port", warshipRequiredPortLevel);
            const nukeCampaignLocked =
                !campaignAllowsAction(fleetUnlocks, "nuke") ||
                !campaignAllowsBuilding(fleetUnlocks, "City", nukeRequiredCityLevel);
            const tradeUnlocked = !tradeCampaignLocked && portLevels >= tradeRequiredPortLevel;
            const warshipUnlocked = !warshipCampaignLocked && portLevels >= warshipRequiredPortLevel;
            const nukeUnlocked = !nukeCampaignLocked && cityLevel >= nukeRequiredCityLevel;
            const militaryFull = Number(fleetPanel.military_used) >= Number(fleetPanel.military_capacity);
            const warshipSelected = Boolean(hud.selected_warship_build);
            if (hudRefs!.fleetTrade) {
                const hideTrade = fleetUnlocks !== null && tradeCampaignLocked;
                hudRefs!.fleetTrade.hidden = hideTrade;
                hudRefs!.fleetTrade.style.display = hideTrade ? "none" : "";
                const tradeCount = Math.floor(Number(fleetPanel.trade_ships) || 0);
                const tradeCapacity = Math.floor(Number(fleetPanel.trade_capacity) || 0);
                let tradeStatus = SOW_t("hud.map_action_fleet") + " · " + tradeCount + "/" + tradeCapacity;
                const tradeReason = !tradeUnlocked
                    ? SOW_t("hud.map_action_port") + " " + tradeRequiredPortLevel
                    : "";
                if (tradeReason) tradeStatus += " · " + tradeReason;
                hudRefs!.fleetTrade.setAttribute("aria-label", tradeStatus);
                hudRefs!.fleetTrade.title = tradeStatus;
                hudRefs!.fleetTrade.classList.toggle("is-locked", !tradeUnlocked);
                setCampaignLock(hudRefs!.fleetTrade, fleetUnlocks === null && !tradeUnlocked);
            }
            if (hudRefs!.fleetWarship) {
                const hideWarship = fleetUnlocks !== null && warshipCampaignLocked;
                hudRefs!.fleetWarship.hidden = hideWarship;
                hudRefs!.fleetWarship.style.display = hideWarship ? "none" : "";
                const warshipCost = Math.max(0, Number(fleetPanel.warship_cost) || 0);
                const warshipDisabled =
                    warshipCampaignLocked || !warshipUnlocked || militaryFull || gold < warshipCost;
                hudRefs!.fleetWarship.disabled = warshipDisabled;
                hudRefs!.fleetWarship.setAttribute("aria-disabled", String(warshipDisabled));
                hudRefs!.fleetWarship.setAttribute("aria-pressed", String(warshipSelected));
                hudRefs!.fleetWarship.classList.toggle("active", warshipSelected);
                hudRefs!.fleetWarship.classList.toggle("is-locked", warshipDisabled && !warshipSelected);
                const warshipReason = !warshipUnlocked
                    ? SOW_t("lobbies.locked") +
                      " · " +
                      SOW_t("hud.map_action_port") +
                      " " +
                      warshipRequiredPortLevel
                    : militaryFull
                      ? Math.floor(Number(fleetPanel.military_used) || 0) +
                        "/" +
                        Math.floor(Number(fleetPanel.military_capacity) || 0)
                      : gold < warshipCost
                        ? SOW_t("hud.not_enough_gold", {
                              cost: Math.ceil(warshipCost).toLocaleString(),
                              available: Math.floor(gold).toLocaleString(),
                          })
                        : SOW_t("hud.map_action_port") +
                          " " +
                          warshipRequiredPortLevel +
                          " · " +
                          Math.floor(warshipCost).toLocaleString() +
                          "g";
                const warshipLabel = SOW_t("hud.map_action_build_warship") + " · " + warshipReason;
                hudRefs!.fleetWarship.setAttribute("aria-label", warshipLabel);
                hudRefs!.fleetWarship.title = warshipLabel;
                setCampaignLock(hudRefs!.fleetWarship, fleetUnlocks === null && warshipDisabled);
                setTileCost(hudRefs!.fleetWarship, warshipCost);
            }
            if (hudRefs!.fleetNuke) {
                const hideNuke = fleetUnlocks !== null && nukeCampaignLocked;
                hudRefs!.fleetNuke.hidden = hideNuke;
                hudRefs!.fleetNuke.style.display = hideNuke ? "none" : "";
                const cooldownTicks = Math.max(0, Number(fleetPanel.nuke_cooldown_ticks) || 0);
                const nukeCost = Math.max(0, Number(fleetPanel.nuke_cost) || 0);
                const nukeReady =
                    Boolean(fleetPanel.nuke_available) && cooldownTicks === 0 && gold >= nukeCost;
                const nukeDisabled = nukeCampaignLocked || !nukeUnlocked || !nukeReady;
                hudRefs!.fleetNuke.disabled = nukeDisabled;
                hudRefs!.fleetNuke.setAttribute("aria-disabled", String(nukeDisabled));
                hudRefs!.fleetNuke.setAttribute("aria-pressed", String(hud.selected_nuke === "AtomBomb"));
                hudRefs!.fleetNuke.classList.toggle("is-locked", nukeDisabled);
                hudRefs!.fleetNuke.classList.toggle("active", hud.selected_nuke === "AtomBomb");
                const nukeReason = !nukeUnlocked
                    ? SOW_t("lobbies.locked") +
                      " · " +
                      SOW_t("hud.map_action_city") +
                      " " +
                      nukeRequiredCityLevel
                    : cooldownTicks > 0
                      ? SOW_t("lobbies.locked") + " · " + Math.ceil(cooldownTicks / 10) + "s"
                      : gold < nukeCost
                        ? SOW_t("hud.not_enough_gold", {
                              cost: Math.ceil(nukeCost).toLocaleString(),
                              available: Math.floor(gold).toLocaleString(),
                          })
                        : !nukeReady
                          ? SOW_t("lobbies.locked")
                          : SOW_t("hud.map_action_nuke") +
                            " · " +
                            Math.floor(nukeCost).toLocaleString() +
                            "g";
                const nukeLabel = SOW_t("hud.map_action_nuke") + " · " + nukeReason;
                hudRefs!.fleetNuke.setAttribute("aria-label", nukeLabel);
                hudRefs!.fleetNuke.title = nukeLabel;
                setCampaignLock(hudRefs!.fleetNuke, fleetUnlocks === null && nukeDisabled);
                setTileCost(hudRefs!.fleetNuke, nukeCost);
            }
            hasFleetItems =
                (hudRefs!.fleetTrade && !hudRefs!.fleetTrade.hidden) ||
                (hudRefs!.fleetWarship && !hudRefs!.fleetWarship.hidden) ||
                (hudRefs!.fleetNuke && !hudRefs!.fleetNuke.hidden);
            hudRefs!.fleetStrip.style.display = isDeploying || !hasFleetItems ? "none" : "flex";
        }

        syncEmoteTexts(hud);
        if (hudRefs!.devSidebar) {
            hudRefs!.devSidebar.classList.toggle("hidden", !showDevTools || !devSidebarOpen);
        }
        if (hudRefs!.devBtn) {
            hudRefs!.devBtn.classList.toggle("active", devSidebarOpen);
        }

        if (hudRefs!.settings) {
            hudRefs!.settings.classList.toggle("hidden", !settingsOpen);
            if (settingsOpen && hudState.settings) {
                const settings = hudState.settings;
                const muteInput = hudRefs!.settings.querySelector<HTMLInputElement>(
                    '[data-hud-setting="mute_all"]',
                );
                const musicInput = hudRefs!.settings.querySelector<HTMLInputElement>(
                    '[data-hud-setting="music_volume"]',
                );
                const motionInput = hudRefs!.settings.querySelector<HTMLInputElement>(
                    '[data-hud-setting="reduced_motion"]',
                );
                const freeZoomInput = hudRefs!.settings.querySelector<HTMLInputElement>(
                    '[data-hud-setting="free_zoom_out"]',
                );
                const fpsPingInput = hudRefs!.settings.querySelector<HTMLInputElement>(
                    '[data-hud-setting="show_fps_ping"]',
                );
                const devToolsInput = hudRefs!.settings.querySelector<HTMLInputElement>(
                    '[data-hud-setting="show_dev_tools"]',
                );
                if (muteInput && document.activeElement !== muteInput) muteInput.checked = !settings.mute_all;
                if (musicInput && document.activeElement !== musicInput) {
                    const musicVol = settings.music_volume == null ? 0.8 : settings.music_volume;
                    musicInput.value = String(musicVol);
                    const pct = Math.round(Math.max(0, Math.min(1, Number(musicVol))) * 100);
                    musicInput.style.setProperty("--sow-music-fill", pct + "%");
                    const musicVal = hudRefs!.settings.querySelector<HTMLElement>("#sow-hud-music-value");
                    if (musicVal) musicVal.textContent = pct + "%";
                }
                if (motionInput && document.activeElement !== motionInput)
                    motionInput.checked = Boolean(settings.reduced_motion);
                if (freeZoomInput && document.activeElement !== freeZoomInput)
                    freeZoomInput.checked = Boolean(settings.free_zoom_out);
                if (fpsPingInput && document.activeElement !== fpsPingInput)
                    fpsPingInput.checked = settings.show_fps_ping === true;
                if (devToolsInput && document.activeElement !== devToolsInput)
                    devToolsInput.checked = Boolean(settings.show_dev_tools);
            }
        }

        if (hudRefs!.inbox) {
            const inboxWasHidden = hudRefs!.inbox.classList.contains("hidden");
            hudRefs!.inbox.classList.toggle("hidden", !inboxOpen);
            if (inboxOpen) {
                renderInbox(hud.inbox || []);
                if (inboxWasHidden && hudRefs!.inboxClose) hudRefs!.inboxClose.focus();
            } else if (
                !inboxWasHidden &&
                hudRefs!.inbox.contains(document.activeElement) &&
                hudRefs!.inboxButton
            ) {
                hudRefs!.inboxButton.focus();
            }
        }
        if (hudRefs!.transfer) {
            const transferWasHidden = hudRefs!.transfer.classList.contains("hidden");
            hudRefs!.transfer.classList.toggle("hidden", !hud.transfer);
            if (hud.transfer) {
                hudRefs!.transfer.dataset.targetId = String(hud.transfer.target_id);
                renderPlayerIdentity(
                    hudRefs!.transferIdentity,
                    hud.transfer.target_name,
                    hud.transfer.target_avatar,
                );
                hudRefs!.transferTarget.setAttribute(
                    "aria-label",
                    SOW_t("hud.target_player", {
                        name: hud.transfer.target_name || SOW_t("hud.player_name"),
                    }),
                );
                if (
                    transferWasHidden ||
                    hudRefs!.transfer.dataset.suggestedTarget !== String(hud.transfer.target_id)
                ) {
                    hudRefs!.transfer.dataset.suggestedTarget = String(hud.transfer.target_id);
                    if (hudRefs!.transferGold)
                        hudRefs!.transferGold.value = String(Math.floor(hud.transfer.suggested_gold || 0));
                    if (hudRefs!.transferTroops)
                        hudRefs!.transferTroops.value = String(
                            Math.floor(hud.transfer.suggested_troops || 0),
                        );
                }
                if (transferWasHidden) {
                    const activeElement = document.activeElement;
                    hudRefs!.transferReturnFocus =
                        activeElement instanceof HTMLElement ? activeElement : null;
                    hudRefs!.transferClose.focus({ preventScroll: true });
                }
            } else if (!transferWasHidden && hudRefs!.transfer.contains(document.activeElement)) {
                restoreHudFocus(hudRefs!.transferReturnFocus);
            }
        }
        if (hudRefs!.betrayal) {
            const betrayalWasHidden = hudRefs!.betrayal.classList.contains("hidden");
            const betrayal = hud.betrayal;
            betrayalOpen = Boolean(betrayal);
            hudRefs!.betrayal.classList.toggle("hidden", !betrayalOpen);
            if (betrayal) {
                renderPlayerIdentity(hudRefs!.betrayalIdentity, betrayal.ally_name, betrayal.ally_avatar);
                if (betrayalWasHidden) {
                    const activeElement = document.activeElement;
                    hudRefs!.betrayalReturnFocus =
                        activeElement instanceof HTMLElement ? activeElement : null;
                    hudRefs!.betrayalCancel.focus({ preventScroll: true });
                }
            } else if (!betrayalWasHidden && hudRefs!.betrayal.contains(document.activeElement)) {
                restoreHudFocus(hudRefs!.betrayalReturnFocus);
            }
        }

        renderNotifications(hud.notifications, forceNotifications, hudScreen);
        renderCombatOperations(hud.operations);
        renderMapFeedback(hud.map_feedback);

        // Leaderboard
        if (hudRefs!.leaderboard) {
            hudRefs!.leaderboard.classList.toggle("hidden", !leaderboardOpen);
        }
        if (leaderboardOpen) {
            if (hudRefs!.lbSearch && hudRefs!.lbSearch.value !== leaderboardFilterQuery) {
                hudRefs!.lbSearch.value = leaderboardFilterQuery;
            }
            updateLeaderboard(
                Array.isArray(hud.leaderboard) ? hud.leaderboard : lastLeaderboardPlayers,
                Array.isArray(hud.team_leaderboard) ? hud.team_leaderboard : lastLeaderboardTeams,
            );
        }

        // Surrender Modal
        if (hudRefs!.surrender) {
            hudRefs!.surrender.classList.toggle("hidden", !surrenderModalOpen);
            const tutorialActive = Boolean(hud.tutorial && hud.tutorial.active);
            const surrenderLeader = leaderById(
                (hud && hud.player_leader) || (hudState && hudState.selected_leader),
            );
            if (surrenderModalOpen && !tutorialActive && surrenderMessage == null) {
                const messages = surrenderMessageKeys.map(function (key) {
                    return SOW_t(key);
                });
                surrenderMessage = messages.length
                    ? messages[Math.floor(Math.random() * messages.length)]
                    : "";
            }
            if (tutorialActive) surrenderMessage = null;
            if (hudRefs!.surrenderTitle)
                hudRefs!.surrenderTitle.textContent = tutorialActive
                    ? SOW_t("endgame.leave_tutorial")
                    : SOW_t("endgame.leave_match");
            if (hudRefs!.surrenderDesc)
                hudRefs!.surrenderDesc.textContent = tutorialActive
                    ? SOW_t("endgame.tutorial_description")
                    : surrenderMessage || SOW_t("endgame.your_battle_will_end");
            if (hudRefs!.surrenderStats) hudRefs!.surrenderStats.classList.toggle("hidden", tutorialActive);
            renderEndgameStats(hudRefs!.surrenderStatValues, hud.player_endgame_stats);
            if (hudRefs!.surrenderSaveNote) {
                hudRefs!.surrenderSaveNote.textContent = SOW_t("endgame.leave_rewards_saved");
                hudRefs!.surrenderSaveNote.classList.toggle("hidden", tutorialActive);
            }
            if (hudRefs!.surrenderPortrait) {
                hudRefs!.surrenderPortrait.src = asset(
                    "gameplay/avatars/" + (surrenderLeader.art_slug || surrenderLeader.slug) + ".webp",
                );
                hudRefs!.surrenderPortrait.alt = surrenderLeader.name || SOW_t("hud.leader_avatar");
            }
            if (hudRefs!.surrenderCancel) hudRefs!.surrenderCancel.textContent = SOW_t("endgame.cancel");
            if (hudRefs!.surrenderActionLabel)
                hudRefs!.surrenderActionLabel.textContent = tutorialActive
                    ? SOW_t("endgame.leave_tutorial")
                    : SOW_t("endgame.leave_match");
        }

        // Endgame Screen
        const isOver = Boolean(hud.match_over);
        const isWinner = Boolean(hud.is_winner);
        if (hudRefs!.endgame) {
            hudRefs!.endgame.classList.toggle("hidden", !isOver);
            if (isOver) {
                const result = isWinner ? "victory" : "defeat";
                const stats = hud.player_endgame_stats || {};
                if (hudRefs!.endgameCard) hudRefs!.endgameCard.dataset.result = result;
                if (hudRefs!.endgameBanner)
                    hudRefs!.endgameBanner.textContent = isWinner
                        ? SOW_t("hud.victory")
                        : SOW_t("hud.defeat");
                const activeLeaderId = (hud && hud.player_leader) || (hudState && hudState.selected_leader);
                const activeLeader = leaderById(activeLeaderId);
                if (hudRefs!.endgamePortrait) {
                    hudRefs!.endgamePortrait.src = asset(
                        "gameplay/avatars/" + (activeLeader.art_slug || activeLeader.slug) + ".webp",
                    );
                    hudRefs!.endgamePortrait.alt = activeLeader.name || SOW_t("hud.leader_avatar");
                }
                renderEndgameStats(hudRefs!.endgameStatValues, stats);
                const featuredSkin =
                    window.SOW_PORTAL === "poki" || window.SOW_PORTAL === "jest" ? null : hud.featured_skin;
                if (hudRefs!.endgameStore) hudRefs!.endgameStore.classList.toggle("hidden", !featuredSkin);
                if (featuredSkin) {
                    if (hudRefs!.endgameStoreName)
                        hudRefs!.endgameStoreName.textContent =
                            featuredSkin.name || SOW_t("store.featured_skin");
                    if (hudRefs!.endgameStoreCopy)
                        hudRefs!.endgameStoreCopy.textContent =
                            SOW_t("store.gems_count", { amount: featuredSkin.cost_gems || 0 }) +
                            " · " +
                            SOW_t("store.original_cosmetic");
                }
                if (hudRefs!.endgameObserve)
                    hudRefs!.endgameObserve.classList.toggle("hidden", isWinner || Boolean(hud.winner_name));
            }
        }
        const mapMenuOpen = renderMapMenu(hud.map_menu);
        renderPlacementPanel(hud.building_placement);
        renderWorldBuildingProgress(hud.buildings_in_progress);
        renderWorldBuildingCelebrations(hud.building_celebrations);
        const panelStates: Record<string, boolean> = {
            toggle_leaderboard: leaderboardOpen,
            toggle_inbox: inboxOpen,
            toggle_settings: settingsOpen,
            toggle_dev_sidebar: devSidebarOpen,
        };
        hudRefs!.panelButtons.forEach(function (button) {
            const command = button.dataset.command;
            const expanded = Boolean(command && panelStates[command]);
            button.classList.toggle("active", expanded);
            const value = String(expanded);
            if (button.getAttribute("aria-expanded") !== value) button.setAttribute("aria-expanded", value);
        });
        hudRoot.dataset.overlayOpen = String(
            Boolean(
                leaderboardOpen ||
                inboxOpen ||
                hud.transfer ||
                betrayalOpen ||
                surrenderModalOpen ||
                isOver ||
                devSidebarOpen ||
                settingsOpen ||
                mapMenuOpen,
            ),
        );
        const dockHidden = !(isDeploying || hasBuildingItems || hasFleetItems);
        if (hudRefs!.dock && hudRefs!.dock.hidden !== dockHidden) {
            hudRefs!.dock.hidden = dockHidden;
            syncCompactHudBounds(hudScreen);
        }
    }

    if (hudRoot) {
        let zoomHold: { button: HTMLButtonElement; repeat: number } | null = null;
        function stopZoomHold() {
            const hold = zoomHold;
            if (!hold) return;
            window.clearInterval(hold.repeat);
            hold.button.classList.remove("is-holding", "is-zoom-limited");
            zoomHold = null;
        }
        function zoomAtLimit(command: string): boolean {
            const zoom = hudState && hudState.hud && hudState.hud.camera_zoom_state;
            if (!zoom || !Number.isFinite(Number(zoom.current))) return false;
            return command === "zoom_out"
                ? Number(zoom.current) <= Number(zoom.floor) + 0.02
                : Number(zoom.current) >= Number(zoom.ceiling) - 0.02;
        }
        hudRoot.addEventListener("pointerdown", function (event) {
            const target = event.target;
            if (!(target instanceof Element)) return;
            const button = target.closest('[data-command="zoom_in"], [data-command="zoom_out"]');
            if (!(button instanceof HTMLButtonElement) || button.disabled || event.button !== 0) return;
            stopZoomHold();
            event.preventDefault();
            event.stopPropagation();
            const command = button.dataset.command;
            if (!command) return;
            button.classList.add("is-holding");
            send(command);
            const hold = { button: button, repeat: 0 };
            zoomHold = hold;
            hold.repeat = window.setInterval(function () {
                if (zoomHold !== hold) return;
                if (zoomAtLimit(command)) {
                    window.clearInterval(hold.repeat);
                    button.classList.add("is-zoom-limited");
                    return;
                }
                send(command);
            }, 180);
        });
        ["pointerup", "pointercancel", "blur"].forEach(function (type) {
            window.addEventListener(type, stopZoomHold, { passive: true });
        });
        hudRoot.addEventListener(
            "pointerdown",
            function (event) {
                const target = event.target;
                if (
                    hudDropdownOpenKey &&
                    target instanceof Element &&
                    !target.closest("[data-control-dropdown]")
                )
                    setHudDropdownOpen(null);
            },
            true,
        );
        hudRoot.addEventListener(
            "scroll",
            function () {
                if (hudDropdownOpenKey) syncDropdownScope(hudRoot, hudDropdownOpenKey);
            },
            true,
        );
        window.addEventListener("resize", function () {
            if (hudDropdownOpenKey) syncDropdownScope(hudRoot, hudDropdownOpenKey);
        });
        function handleHudClick(event: MouseEvent): void {
            const target = event.target;
            if (!(target instanceof Element)) return;
            const emoteChoice = target.closest<HTMLElement>("[data-emote-kind]");
            if (emoteChoice) {
                const emoteKind = emoteChoice.dataset.emoteKind;
                if (!emoteKind) return;
                event.preventDefault();
                event.stopPropagation();
                send("express_emote", {
                    session: Number(hudRefs!.mapMenu!.dataset.session),
                    tile_idx: Number(hudRefs!.mapMenu!.dataset.tileIdx),
                    kind: emoteKind,
                });
                return;
            }
            const pinButton = target.closest<HTMLElement>("[data-emote-pin-toggle]");
            if (pinButton) {
                event.preventDefault();
                event.stopPropagation();
                // The pressed state comes from the game, so the button always flips what the game shows.
                send("set_emote_pinned", { pinned: pinButton.getAttribute("aria-pressed") !== "true" });
                return;
            }
            const placementBtn = target.closest<HTMLButtonElement>(
                "[data-placement-action], [data-placement-nudge]",
            );
            if (placementBtn) {
                event.preventDefault();
                event.stopPropagation();
                if (placementBtn.disabled) return;
                const action = placementBtn.dataset.placementAction;
                const nudge = placementBtn.dataset.placementNudge;
                if (action === "confirm") {
                    send("confirm_building_placement");
                } else if (action === "cancel") {
                    send("clear_placement");
                } else if (nudge) {
                    send("nudge_placement", { direction: nudge });
                }
                return;
            }
            const mapButton = target.closest<HTMLButtonElement>("[data-map-action]");
            const mapGroup = target.closest<HTMLElement>("[data-map-group]");
            const mapBack = target.closest<HTMLButtonElement>("[data-map-back]");
            if (mapButton || mapGroup || mapBack) {
                event.preventDefault();
                event.stopPropagation();
                const mapMenu = hudRefs && hudRefs.mapMenu;
                if (!mapMenu) return;
                if (mapBack) {
                    const fromGroup = mapMenuView;
                    mapMenuView = "root";
                    renderMapMenu(hudState && hudState.hud && hudState.hud.map_menu);
                    const groupSlot = mapMenu.querySelector<HTMLElement>(
                        '[data-map-group="' + fromGroup + '"]',
                    );
                    if (groupSlot) groupSlot.focus({ preventScroll: true });
                } else if (mapGroup) {
                    mapMenuView = mapGroup.dataset.mapGroup === "emote" ? "emote" : "build";
                    renderMapMenu(hudState && hudState.hud && hudState.hud.map_menu);
                    const firstChoice = mapMenu.querySelector<HTMLButtonElement>(
                        ".sow-hud__ring-slot:not(:disabled)",
                    );
                    if (firstChoice) firstChoice.focus({ preventScroll: true });
                } else if (mapButton) {
                    if (mapButton.disabled) return;
                    // Hold-to-confirm actions only fire from a completed hold.
                    if (mapButton.dataset.ringHold === "true" && !ring.holdConfirmed) return;
                    const action = mapButton.dataset.mapAction;
                    if (!action) return;
                    console.info("[RING TRACE] map_menu_action " + action + " trusted=" + event.isTrusted);
                    send("map_menu_action", {
                        session: Number(mapMenu.dataset.session),
                        tile_idx: Number(mapMenu.dataset.tileIdx),
                        action,
                    });
                }
                return;
            }
            const dropdownButton = target.closest<HTMLButtonElement>(
                '[data-command="toggle_dropdown"], [data-command="select_dropdown"]',
            );
            if (dropdownButton && hudRoot && hudRoot.contains(dropdownButton)) {
                event.preventDefault();
                event.stopPropagation();
                const key = dropdownButton.dataset.dropdownKey;
                if (!key) return;
                if (dropdownButton.dataset.command === "toggle_dropdown") {
                    setHudDropdownOpen(hudDropdownOpenKey === key ? null : key);
                } else {
                    const value = dropdownButton.dataset.dropdownOptionValue;
                    if (value != null) selectHudDropdownOption(key, value);
                }
                return;
            }
            const btn = target.closest<HTMLElement>("[data-command]");
            if (!btn) return;
            const cmd = btn.dataset.command;
            if (!cmd) return;
            if (cmd === "zoom_in" || cmd === "zoom_out") {
                if (event.detail === 0) send(cmd);
                return;
            }
            if (cmd === "toggle_dev_sidebar") {
                setHudPanel(null);
                devSidebarOpen = !devSidebarOpen;
                send("toggle_dev_sidebar");
                renderHud();
            } else if (cmd === "toggle_leaderboard") {
                setHudPanel("leaderboard");
                renderHud();
            } else if (cmd === "toggle_inbox") {
                setHudPanel("inbox");
                renderHud();
            } else if (cmd === "toggle_settings") {
                setHudPanel("settings");
                renderHud();
            } else if (cmd === "reset_dev_config") {
                send("reset_dev_config", {
                    section: btn.dataset.devSection || null,
                    field: btn.dataset.devField || null,
                });
            } else if (cmd === "dev_step") {
                if (event.detail === 0 && btn instanceof HTMLButtonElement) devStepOnce(btn);
            } else if (cmd === "dev_tab") {
                devTab = btn.dataset.devTab || "";
                try {
                    window.localStorage.setItem("sow_dev_tab", devTab);
                } catch {
                    /* storage unavailable */
                }
                const devSearch = hudRefs!.devSidebar!.querySelector<HTMLInputElement>("[data-dev-search]");
                if (devSearch) devSearch.value = "";
                renderDevControls((hudState && hudState.hud && hudState.hud.dev_tools) || {});
                const devBody = hudRefs!.devSidebar.querySelector<HTMLElement>(".sow-hud__dev-body");
                if (devBody) devBody.scrollTop = 0;
            } else if (cmd === "dev_menu") {
                const devMenu = hudRefs!.devSidebar!.querySelector<HTMLElement>("#sow-hud-dev-menu");
                if (devMenu) setDevMenu(devMenu.hasAttribute("hidden"));
            } else if (cmd === "dev_preview") {
                send("set_dev_vfx_preview", { target: Number(btn.dataset.devTarget) });
                send("replay_dev_vfx_preview");
            } else if (cmd === "dev_reset_all") {
                if (devResetTimer) {
                    setDevMenu(false);
                    send("reset_dev_config", { section: null, field: null });
                } else {
                    btn.textContent = "Tap again to reset all";
                    devResetTimer = window.setTimeout(disarmDevReset, 3000);
                }
            } else if (cmd === "dev_perf") {
                const group = btn.dataset.devPerf;
                if (group) send("dev_vfx_group", { group, on: btn.dataset.state === "off" });
            } else if (cmd === "dev_vfx_all") {
                setDevMenu(false);
                send("dev_vfx_all", { on: btn.dataset.on === "1" });
            } else if (cmd === "clear_emote") {
                send("clear_emote");
            } else if (cmd === "open_transfer") {
                setHudPanel(null);
                send("open_transfer", { target_player_id: Number(btn.dataset.playerId) });
                renderHud();
            } else if (cmd === "close_transfer") {
                send("close_transfer");
            } else if (cmd === "send_resources" || cmd === "request_resources") {
                const transfer = hudRefs && hudRefs.transfer;
                const targetId = transfer ? Number(transfer.dataset.targetId) : 0;
                let gold = hudRefs && hudRefs.transferGold ? Number(hudRefs.transferGold.value) : 0;
                let transferTroops =
                    hudRefs && hudRefs.transferTroops ? Number(hudRefs.transferTroops.value) : 0;
                if (!Number.isFinite(gold)) gold = 0;
                if (!Number.isFinite(transferTroops)) transferTroops = 0;
                if (targetId > 0)
                    send(cmd, { target_player_id: targetId, gold: gold, troops: transferTroops });
            } else if (
                cmd === "accept_alliance" ||
                cmd === "reject_alliance" ||
                cmd === "accept_resource_request" ||
                cmd === "reject_resource_request"
            ) {
                const requesterId = Number(btn.dataset.playerId);
                if (requesterId > 0) send(cmd, { target_player_id: requesterId });
            } else if (cmd === "cancel_betrayal") {
                betrayalOpen = false;
                pendingCounterAttacks = Object.create(null);
                send("cancel_betrayal");
                renderHud();
            } else if (cmd === "confirm_betrayal") {
                betrayalOpen = false;
                send("confirm_betrayal");
                renderHud();
            } else if (cmd === "cancel_attack" || cmd === "recall_fleet" || cmd === "scuttle_fleet") {
                const isAttack = cmd === "cancel_attack";
                const id = Number(isAttack ? btn.dataset.attackId : btn.dataset.fleetId);
                if (id > 0 && btn instanceof HTMLButtonElement)
                    btn.disabled = send(cmd, isAttack ? { attack_id: id } : { fleet_id: id });
            } else if (cmd === "counter_attack") {
                const counterTarget = Number(btn.dataset.playerId);
                const incomingTotal = Number(btn.dataset.incomingTroops);
                if (counterTarget > 0 && incomingTotal > 0 && pendingCounterAttacks[counterTarget] == null) {
                    pendingCounterAttacks[counterTarget] = incomingTotal;
                    if (hudRefs && hudRefs.operationRows) {
                        hudRefs.operationRows
                            .querySelectorAll<HTMLButtonElement>('[data-command="counter_attack"]')
                            .forEach(function (button) {
                                if (Number(button.dataset.playerId) === counterTarget) button.disabled = true;
                            });
                    }
                    if (!send("counter_attack", { target_player_id: counterTarget })) {
                        delete pendingCounterAttacks[counterTarget];
                        if (hudRefs && hudRefs.operationRows) {
                            hudRefs.operationRows
                                .querySelectorAll<HTMLButtonElement>('[data-command="counter_attack"]')
                                .forEach(function (button) {
                                    if (Number(button.dataset.playerId) === counterTarget)
                                        button.disabled = false;
                                });
                        }
                    }
                }
            } else if (cmd === "focus_world") {
                const focusX = Number(btn.dataset.focusX);
                const focusY = Number(btn.dataset.focusY);
                if (Number.isFinite(focusX) && Number.isFinite(focusY))
                    send("focus_world", { x: focusX, y: focusY });
            } else if (cmd === "prompt_surrender") {
                send("request_endgame_stats");
                surrenderModalOpen = true;
                surrenderMessage = null;
                renderHud();
            } else if (cmd === "close_surrender_modal") {
                send("clear_endgame_stats");
                surrenderModalOpen = false;
                surrenderMessage = null;
                renderHud();
            } else if (cmd === "confirm_surrender") {
                surrenderModalOpen = false;
                surrenderMessage = null;
                send("return_to_menu");
                renderHud();
            } else if (cmd === "zoom_in") {
                send("zoom_in");
            } else if (cmd === "zoom_out") {
                send("zoom_out");
            } else if (cmd === "center_camera") {
                send("center_camera");
            } else if (cmd === "focus_player") {
                const playerId = btn.dataset.playerId;
                if (!playerId) return;
                const pid = parseInt(playerId, 10);
                if (!isNaN(pid)) {
                    send("focus_player", { player_id: pid });
                }
            } else if (cmd === "spawn_troops") {
                send("spawn_troops");
            } else if (cmd === "select_building") {
                const kind = btn.dataset.kind;
                if (kind) send("select_building", { kind });
            } else if (cmd === "select_warship") {
                send("select_warship");
            } else if (cmd === "select_nuke") {
                send("select_nuke");
            } else if (cmd === "confirm_endgame_leave") {
                send("return_to_menu");
            } else if (cmd === "open_store") {
                if (window.SOW_PORTAL === "poki" || window.SOW_PORTAL === "jest") return;
                send("return_to_menu", { destination: "store" });
            } else if (cmd === "continue_observing") {
                send("continue_observing");
            }
        }
        hudRoot.addEventListener("click", handleHudClick);
        hudDevLayer!.addEventListener("click", handleHudClick);
        bindRingInput();
    }

    function handleHudStateUpdate(raw: string, parsed: HudStateEnvelope): void {
        if (typeof raw !== "string" || raw === lastHudRaw) return;
        lastHudRaw = raw;
        const previousHud = hudState && hudState.phase === "Playing" ? hudState.hud : null;
        hudState = parsed;
        if (hudState.phase === "Playing") {
            if (!previousHud) emoteTextKey = "";
            hudState.hud = hudState.hud || previousHud;
            // Hover arrives on its own channel (SOW_hover_card_update), so keep the last one.
            if (hudState.hud && previousHud) hudState.hud.hovered = previousHud.hovered;
            // The tutorial pointer arrives on its own channel (SOW_tutorial_pointer_update), so keep the last one.
            if (hudState.hud && hudState.hud.tutorial && previousHud && previousHud.tutorial)
                hudState.hud.tutorial.pointer = previousHud.tutorial.pointer;
            if (hudState.hud && hudState.hud.dev_tools && typeof hudState.hud.dev_tools.open === "boolean") {
                devSidebarOpen = hudState.hud.dev_tools.open;
            }
        }
        renderHud();
        if (typeof window.SOW_tutorial_state_update === "function") {
            window.SOW_tutorial_state_update(hudState);
        }
    }

    window.SOW_onStateUpdate = function (raw) {
        if (typeof raw !== "string") return;
        let parsed: HudStateEnvelope;
        try {
            parsed = JSON.parse(raw) as HudStateEnvelope;
        } catch (error) {
            console.warn("[WEB STATE] invalid state:", error);
            return;
        }
        if (typeof window.SOW_menu_state_update === "function") {
            window.SOW_menu_state_update(raw, parsed);
        }
        handleHudStateUpdate(raw, parsed);
    };

    window.SOW_hover_card_update = function (raw) {
        if (!hudState || hudState.phase !== "Playing" || !hudState.hud) return;
        const hovered = JSON.parse(raw);
        hudState.hud.hovered = hovered;
        renderHoverCard(hovered);
        if (typeof window.SOW_tutorial_state_update === "function") {
            window.SOW_tutorial_state_update(hudState);
        }
    };

    window.SOW_tutorial_pointer_update = function (raw) {
        if (!hudState || hudState.phase !== "Playing" || !hudState.hud || !hudState.hud.tutorial) return;
        hudState.hud.tutorial.pointer = JSON.parse(raw);
        if (typeof window.SOW_tutorial_state_update === "function") {
            window.SOW_tutorial_state_update(hudState);
        }
    };

    if (window.SOW_I18N_READY && typeof window.SOW_I18N_READY.then === "function") {
        window.SOW_I18N_READY.then(function () {
            emoteTextKey = "";
            syncEmoteTexts(hudState && hudState.hud);
        });
    }
    if (hudRoot)
        hudRoot.addEventListener("keydown", function (event) {
            if (handleHudDropdownKeydown(event)) return;
            if (
                (event.key === "Enter" || event.key === " ") &&
                event.target instanceof HTMLElement &&
                event.target.tagName !== "INPUT" &&
                event.target.tagName !== "TEXTAREA"
            ) {
                const roleButton = event.target.closest<HTMLElement>('[role="button"][data-command]');
                if (roleButton && roleButton.tagName !== "BUTTON") {
                    event.preventDefault();
                    roleButton.click();
                    return;
                }
            }
            if (hudRefs && hudRefs.betrayal && !hudRefs.betrayal.classList.contains("hidden")) {
                if (event.key === "Escape") {
                    event.preventDefault();
                    event.stopPropagation();
                    pendingCounterAttacks = Object.create(null);
                    send("cancel_betrayal");
                    return;
                }
                if (event.key === "Tab") {
                    const buttons = Array.from(
                        hudRefs.betrayal.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"),
                    );
                    const index = buttons.findIndex((button) => button === document.activeElement);
                    if (
                        buttons.length &&
                        (index < 0 ||
                            (event.shiftKey && index === 0) ||
                            (!event.shiftKey && index === buttons.length - 1))
                    ) {
                        event.preventDefault();
                        (event.shiftKey ? buttons[buttons.length - 1] : buttons[0]).focus({
                            preventScroll: true,
                        });
                    }
                    return;
                }
            }
            if (
                event.key === "Escape" &&
                hudRefs &&
                hudRefs.placementOverlay &&
                !hudRefs.placementOverlay.classList.contains("hidden")
            ) {
                event.preventDefault();
                event.stopPropagation();
                send("clear_placement");
                return;
            }
            if (
                event.key === "Escape" &&
                hudRefs &&
                hudRefs.transfer &&
                !hudRefs.transfer.classList.contains("hidden")
            ) {
                event.preventDefault();
                event.stopPropagation();
                send("close_transfer");
                return;
            }
            if (event.key === "Escape" && activeHudPanel === "leaderboard") {
                if (document.activeElement === hudRefs?.lbSearch && hudRefs?.lbSearch.value) {
                    hudRefs.lbSearch.value = "";
                    leaderboardFilterQuery = "";
                    applyLeaderboardFilter();
                    event.preventDefault();
                    event.stopPropagation();
                    return;
                }
                event.preventDefault();
                event.stopPropagation();
                setHudPanel(null);
                renderHud();
                return;
            }
            if (event.key === "Escape" && activeHudPanel === "inbox") {
                event.preventDefault();
                event.stopPropagation();
                setHudPanel(null);
                renderHud();
                return;
            }
            const target = event.target;
            const menu = hudRefs && hudRefs.mapMenu;
            if (!menu || menu.classList.contains("hidden") || !menu.contains(event.target as Node)) return;
            if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                const back = menu.querySelector<HTMLButtonElement>("[data-map-back]");
                if (back) back.click();
                else send("close_map_context_menu");
            } else if (["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight"].indexOf(event.key) >= 0) {
                // Arrows walk the ring clockwise/counter-clockwise, skipping empty and blocked slots.
                const choices = Array.from(
                    menu.querySelectorAll<HTMLButtonElement>(".sow-hud__ring-slot:not(:disabled)"),
                );
                if (!choices.length) return;
                event.preventDefault();
                event.stopPropagation();
                const direction = event.key === "ArrowUp" || event.key === "ArrowLeft" ? -1 : 1;
                let index = choices.findIndex((choice) => choice === document.activeElement);
                if (index < 0) index = direction > 0 ? -1 : 0;
                choices[(index + direction + choices.length) % choices.length].focus({ preventScroll: true });
            } else if (
                (event.key === "Enter" || event.key === " ") &&
                target instanceof HTMLElement &&
                target.dataset.ringHold === "true"
            ) {
                event.preventDefault();
                if (!event.repeat) ringHoldStart(menu, Number(target.dataset.slot));
            }
        });
    if (hudRoot)
        hudRoot.addEventListener("keyup", function (event) {
            const menu = openRingMenu();
            const target = event.target;
            if (
                !menu ||
                (event.key !== "Enter" && event.key !== " ") ||
                !(target instanceof HTMLElement) ||
                target.dataset.ringHold !== "true"
            )
                return;
            event.preventDefault();
            ringActivate(menu, Number(target.dataset.slot));
        });
    if (hudRoot)
        hudRoot.addEventListener("focusin", function (event) {
            const menu = openRingMenu();
            const target = event.target;
            if (menu && target instanceof HTMLElement && menu.contains(target) && target.dataset.slot != null)
                setRingHot(menu, Number(target.dataset.slot));
        });

    window.addEventListener("sow:locale-change", function () {
        emoteTextKey = "";
        if (!hudState) return;
        const active = document.activeElement;
        const transferDraft =
            hudRefs && hudRefs.transfer && !hudRefs.transfer.classList.contains("hidden")
                ? {
                      target: hudRefs.transfer.dataset.targetId,
                      gold: hudRefs.transferGold!.value,
                      troops: hudRefs.transferTroops!.value,
                      focus: hudRefs.transfer.contains(active) ? active!.id : null,
                      returnFocus: hudRefs.transferReturnFocus,
                  }
                : null;
        const betrayalFocus =
            hudRefs && hudRefs.betrayal && hudRefs.betrayal.contains(active) ? active!.id : null;
        const betrayalReturnFocus = hudRefs && hudRefs.betrayalReturnFocus;
        const languageFocus =
            active instanceof HTMLElement &&
            Boolean(
                hudRefs?.settings?.contains(active) &&
                active.closest("[data-dropdown-key='hud-settings-language']"),
            );
        hudDropdownOpenKey = null;
        hudInitialized = false;
        hudRefs = null;
        activeProgressBars.clear();
        activeCelebrations.clear();
        if (hudRoot) hudRoot.replaceChildren();
        if (hudDevLayer && hudDevLayer.parentNode) {
            const freshDevLayer = hudDevLayer.cloneNode(false) as HTMLElement;
            hudDevLayer.parentNode.replaceChild(freshDevLayer, hudDevLayer);
            hudDevLayer = freshDevLayer;
        }
        renderHud(true);
        const refreshedRefs = hudRefs as HudRefs | null;
        if (
            transferDraft &&
            refreshedRefs &&
            refreshedRefs.transfer.dataset.targetId === transferDraft.target
        ) {
            refreshedRefs.transferGold.value = transferDraft.gold;
            refreshedRefs.transferTroops.value = transferDraft.troops;
            refreshedRefs.transferReturnFocus = transferDraft.returnFocus;
            if (transferDraft.focus) restoreHudFocus(document.getElementById(transferDraft.focus));
        }
        if (betrayalFocus && refreshedRefs) {
            refreshedRefs.betrayalReturnFocus = betrayalReturnFocus;
            restoreHudFocus(document.getElementById(betrayalFocus));
        }
        if (languageFocus && refreshedRefs) {
            refreshedRefs.settings
                .querySelector<HTMLElement>(
                    "[data-dropdown-key='hud-settings-language'] [data-role='dropdown-trigger']",
                )
                ?.focus({ preventScroll: true });
        }
    });

    window.addEventListener("resize", updateHudSafeArea);
    updateHudSafeArea();
    if (hudRoot) hudRoot.hidden = true;
    if (hudDevLayer) hudDevLayer.hidden = true;
    if (typeof window.SOW_MENU_STATE === "string") window.SOW_onStateUpdate(window.SOW_MENU_STATE);
})();
