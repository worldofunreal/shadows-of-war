import { formatCompactNumber as formatEndgameCompact } from "./number-format.ts";
import { AudioCue, playUiCue, syncAudioSettings } from "./audio/index.ts";
import {
  type AvatarExpression,
  avatarAssetUrl,
  avatarPortraitPath,
  clearAvatarPortrait,
  loadAvatarImage,
  setAvatarImage,
  setAvatarPortrait,
} from "./avatar-presentation.ts";
import { abilityIconMarkup, browserZoomInputMode, esc } from "./main_menu.core.ts";
import { computeEndgameStatRollups } from "./main_menu.motion.ts";
import { renderLocaleSettingRow } from "./main_menu.settings.ts";
import {
  createRequestToastDeckState,
  dismissAllRequestToasts,
  dismissFrontRequestToast,
  dismissRequestToastByKey,
  isValidInboxRequest,
  readRequestCountdown,
  reconcileRequestToastDeck,
  requestKey,
  resetRequestToastDeck,
  resolveRequestToastMaxVisible,
  syncRequestCountdowns,
  type RequestCountdownRecord,
  type RequestToastDeckState,
  type RequestToastEntry,
} from "./request-toast-deck.ts";
import { syncDropdownScope } from "./sow-dropdown.ts";
import { SOWTooltip } from "./sow-tooltip.ts";
import {
  resolveTransferHotkey,
  resolveTransferMaxTarget,
} from "./transfer-hotkeys.ts";
import type {
  HudAvatarIdentity,
  HudAvatarTransition,
  HudBuildingPlacement,
  HudBuildingProgressItem,
  HudBuildingCelebrationItem,
  HudBuildingProduction,
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
const DEFAULT_ATTACK_RATIO = 0.25;
type HudCommandValue = string | number | boolean | null | number[] | string[];
type HudCommandPayload = Record<string, HudCommandValue>;
type HudScreenSize = { width: number; height: number };
type HudEndgameStats = NonNullable<MenuHudPayload["player_endgame_stats"]>;
type HudEndgameRefs = Record<
  "kills" | "assists" | "maxTerritory" | "goldReceived" | "troopsReceived",
  HTMLElement | null
>;
type HudNotificationAvatar = {
  wrapper: HTMLElement;
  image: HTMLImageElement;
  emblem: HTMLElement;
};
type HudNotificationCard = {
  card: HTMLElement;
  seal: HTMLElement;
  avatars: HudNotificationAvatar[];
  locate: HTMLButtonElement;
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
type ActiveMapFeedback = {
  entry: HudMapFeedback;
  priority: number;
  expiresAt: number;
};
type NotificationRenderItem = {
  entry: HudNotification | HudMapFeedback;
  priority: number;
};
type PlayerIdentityParts = {
  element: HTMLElement;
  portrait: HTMLImageElement;
  emblem: HTMLElement;
  copy: HTMLElement;
  name: HTMLElement;
};
// What the requester asks for, then what a trade gives back.
type InboxAmount = "gold" | "troops" | "pay_gold" | "pay_troops";
type InboxRequestParts = PlayerIdentityParts & {
  type: HTMLElement;
  timer: HTMLElement;
  progressFill: HTMLElement;
  resources: HTMLElement;
  amounts: Record<InboxAmount, { element: HTMLElement; value: HTMLElement }>;
  swap: HTMLElement;
  accept: HTMLButtonElement;
  reject: HTMLButtonElement;
  requestKey: string;
  patchedSignature: string;
  lastTimerSeconds: number;
  lastTimerUrgent: boolean | null;
  countdownAnchorKey: string;
  countdownAnimAlt: boolean;
};
type RequestToastCardParts = InboxRequestParts & {
  stackIndex: number;
};
type LeaderboardSortMode =
  | "rank"
  | "troops"
  | "gold"
  | "troop_rate"
  | "gold_rate"
  | "kills"
  | "alpha";
type LeaderboardRow = {
  card: HTMLElement;
  rank: HTMLElement;
  avatarWrap: HTMLElement;
  avatarImg: HTMLImageElement;
  avatarBadge: HTMLElement;
  name: HTMLElement;
  botBadge?: HTMLElement;
  duelistBadge?: HTMLElement;
  territory: HTMLElement;
  troops: HTMLElement;
  gpsBtn: HTMLButtonElement;
  transferBtn: HTMLButtonElement;
  key: string;
};
type LeaderboardTeamRow = {
  card: HTMLElement;
  rank: HTMLElement;
  name: HTMLElement;
  territory: HTMLElement;
  key: string;
};
type LeaderboardUpdate = {
  players: HudLeaderboardPlayer[];
  teams: HudLeaderboardTeam[];
  playerPositions: Map<string, number>;
  teamPositions: Map<string, number>;
  playerIndex: number;
  teamIndex: number;
  stalePlayerIds: string[];
  stalePlayerIndex: number;
  staleTeamIds: string[];
  staleTeamIndex: number;
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
  presentation:
    | { state: string; key: string; icon: string; disabled: boolean }
    | null
    | undefined;
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
  Pick<
    MenuHudPayload,
    | "emotes"
    | "emote_active"
    | "emote_pinned"
    | "help_cooldown_seconds"
    | "player_leader"
    | "gold"
  >
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
  platePendingTroops: HTMLElement;
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
  deployFill: HTMLElement;
  deployStatus: HTMLElement;
  awaitingPlayers: HTMLElement;
  buildingsStrip: HTMLElement;
  worldProgressOverlay: HTMLElement;
  worldCelebrationsOverlay: HTMLElement;
  buildingButtons: HTMLButtonElement[];
  panelButtons: HTMLButtonElement[];
  leaderboard: HTMLElement;
  rows: HTMLElement;
  lbSearch: HTMLInputElement;
  lbEmpty: HTMLElement;
  lbToolbar: HTMLElement;
  lbTeamWrap: HTMLElement;
  inbox: HTMLElement;
  inboxRows: HTMLElement;
  inboxFilters: HTMLElement;
  inboxFilterButtons: HTMLButtonElement[];
  inboxBulk: HTMLElement;
  inboxAcceptAll: HTMLButtonElement;
  inboxRejectAll: HTMLButtonElement;
  inboxClose: HTMLElement;
  inboxButton: HTMLElement;
  inboxTotal: HTMLElement;
  inboxEmpty: HTMLElement;
  inboxRequests: Record<string, InboxRequestParts>;
  inboxRequestKeys: string[];
  inboxRenderKey: string;
  requestToasts: HTMLDivElement;
  requestToastStack: HTMLDivElement;
  requestToastMore: HTMLButtonElement;
  requestToastCards: RequestToastCardParts[];
  transfer: HTMLElement;
  transferTitle: HTMLElement;
  transferTarget: HTMLElement;
  transferClose: HTMLElement;
  transferReturnFocus: HTMLElement | null;
  transferGold: HTMLInputElement;
  transferTroops: HTMLInputElement;
  transferHint: HTMLElement;
  transferSend: HTMLButtonElement;
  transferRequest: HTMLButtonElement;
  transferTrade: HTMLButtonElement;
  transferDirTroops: HTMLButtonElement;
  transferDirGold: HTMLButtonElement;
  transferStockTroops: HTMLElement;
  transferStockMaxTroops: HTMLElement;
  transferStockGold: HTMLElement;
  transferIdentity: PlayerIdentityParts;
  betrayal: HTMLElement;
  betrayalTitle: HTMLElement;
  betrayalCopy: HTMLElement;
  betrayalTarget: HTMLElement;
  betrayalCancel: HTMLElement;
  betrayalConfirmLabel: HTMLElement;
  betrayalReturnFocus: HTMLElement | null;
  betrayalIdentity: PlayerIdentityParts;
  surrender: HTMLElement;
  surrenderTitle: HTMLElement;
  surrenderStats: HTMLElement;
  surrenderStatValues: HudEndgameRefs;
  surrenderPortrait: HTMLImageElement;
  surrenderCancel: HTMLElement;
  surrenderActionLabel: HTMLElement;
  endgame: HTMLElement;
  endgameCard: HTMLElement;
  endgameBanner: HTMLElement;
  endgamePortrait: HTMLImageElement;
  endgameStatValues: HudEndgameRefs;
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
  if (!(element instanceof HTMLElement))
    throw new Error("Missing HUD element: " + id);
  return element;
}

function requiredHudInput(id: string): HTMLInputElement {
  const element = requiredHudElement(id);
  if (!(element instanceof HTMLInputElement))
    throw new Error("HUD element is not an input: " + id);
  return element;
}

function requiredHudImage(id: string): HTMLImageElement {
  const element = requiredHudElement(id);
  if (!(element instanceof HTMLImageElement))
    throw new Error("HUD element is not an image: " + id);
  return element;
}

function requiredHudButton(id: string): HTMLButtonElement {
  const element = requiredHudElement(id);
  if (!(element instanceof HTMLButtonElement))
    throw new Error("HUD element is not a button: " + id);
  return element;
}

function requiredHudButtonQuery(
  root: ParentNode,
  selector: string,
): HTMLButtonElement {
  const element = root.querySelector(selector);
  if (!(element instanceof HTMLButtonElement))
    throw new Error("Missing HUD button: " + selector);
  return element;
}

function requiredHudQuery(root: ParentNode, selector: string): HTMLElement {
  const element = root.querySelector(selector);
  if (!(element instanceof HTMLElement))
    throw new Error("Missing HUD element: " + selector);
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
  let wasEndgameOpen = false;
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
  let leaderboardTeamRows: Record<string, LeaderboardTeamRow> =
    Object.create(null);
  let leaderboardWork: LeaderboardUpdate | null = null;
  let leaderboardFrame = 0;
  let leaderboardPanelOpen = false;
  let leaderboardOpenedAtMs = 0;
  let leaderboardSyncRetryAtMs = 0;
  // When we may ask each target again: the same ~10s the server enforces per pair.
  const TRANSFER_REQUEST_COOLDOWN_MS = 10_000;
  const transferRequestReadyAt: Record<number, number> = Object.create(null);
  let leaderboardRenderKey = "";
  let leaderboardFilterQuery = "";
  let leaderboardTeamFilter: "all" | "Red" | "Blue" = "all";
  let leaderboardTypeFilter: "all" | "humans" | "bots" = "all";
  let leaderboardSortMode: LeaderboardSortMode = "rank";
  let leaderboardIs1v1 = false;
  let pendingCounterAttacks: Record<string, number> = Object.create(null);
  let lastLeaderboardTeams: HudLeaderboardTeam[] = [];
  let lastLeaderboardPlayersPayload: HudLeaderboardPlayer[] | null = null;
  let lastLeaderboardTeamsPayload: HudLeaderboardTeam[] | null = null;
  let notificationCursor = 0;
  let notificationTimer: number | null = null;
  let bellRingTimer: number | null = null;
  let lastInboxCount = -1;
  let lastAllianceInboxCount = -1;
  let inboxDataVersion = 0;
  let pendingFreshInbox = false;
  let requestToastStateVersion = 0;
  let requestToastRenderKey = "";
  let lastRequestToastsVisible: boolean | null = null;
  let lastRequestToastsStackCount = -1;
  let cachedIntegerLocale = "";
  let cachedIntegerFormatter: Intl.NumberFormat | null = null;
  let visibilityListenerBound = false;
  const requestCountdowns = new Map<string, RequestCountdownRecord>();
  const requestToastDeck: RequestToastDeckState = createRequestToastDeckState();
  type RequestToastExitMode = "accept" | "dismiss" | "expire";
  const pendingToastExitModes = new Map<string, RequestToastExitMode>();
  const leavingToastCards = new Map<
    string,
    { element: HTMLElement; timer: number | null }
  >();
  let requestToastExpiryTimer: number | null = null;
  let requestCountdownTimer: number | null = null;
  type InboxCategoryFilter = "all" | "alliance" | "aid";
  let inboxFilter: InboxCategoryFilter = "all";
  let latestInboxRequests: HudInboxRequest[] = [];
  let activeNotifications: ActiveHudNotification[] = [];
  let notificationCards: HudNotificationCard[] = [];
  const notificationCardsByKey = new Map<string, HudNotificationCard>();
  const leavingNotificationCards = new Map<
    string,
    { parts: HudNotificationCard; timer: number | null }
  >();
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
    slots: [] as RingSlot[],
    press: null as RingPress | null,
    hold: null as RingHold | null,
    holdConfirmed: false,
    pointerDown: false,
    pointerId: null as number | null,
  };
  const allocationHoverNone = window.matchMedia
    ? window.matchMedia("(hover: none)")
    : null;
  let lastAllocationKey = "";
  const EMPTY_CAMPAIGN_UNLOCKS: {
    buildings: Record<string, number>;
    actions: string[];
  } = {
    buildings: {},
    actions: [],
  };
  const DEFAULT_HUD_SETTINGS: MenuSettings = {
    mute_all: false,
    music_volume: 0.8,
    sfx_volume: 0.8,
    reduced_motion: false,
    free_zoom_out: false,
    show_fps_ping: false,
    show_match_clock: false,
    show_dev_tools: false,
    show_dev_tools_available: false,
  };

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
    hudSafeArea = [
      style.paddingTop,
      style.paddingRight,
      style.paddingBottom,
      style.paddingLeft,
    ].map(function (value) {
      return Math.max(0, parseFloat(value) || 0);
    });
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
    if (key === emoteTextKey && emoteTextTarget === window.SOW_menu_command)
      return;
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
    if (previous === "leaderboard" && next !== previous)
      send("toggle_leaderboard", { open: false });
    if (previous === "inbox" && next !== previous)
      send("toggle_inbox", { open: false });
    if (previous !== next) {
      playUiCue(next ? AudioCue.UiModalOpen : AudioCue.UiModalClose);
    }
    activeHudPanel = next;
    if (next && devSidebarOpen) {
      devSidebarOpen = false;
      send("toggle_dev_sidebar");
    }
    if (next === "leaderboard") {
      leaderboardOpenedAtMs = Date.now();
      leaderboardSyncRetryAtMs = 0;
      send("toggle_leaderboard", { open: true });
    }
    if (next === "inbox") send("toggle_inbox", { open: true });
  }

  function setHudDropdownOpen(
    key: string | null,
    focusTarget?: HTMLElement | null,
  ) {
    hudDropdownOpenKey = key;
    if (hudRoot) syncDropdownScope(hudRoot, key);
    if (focusTarget) focusTarget.focus({ preventScroll: true });
  }

  function selectHudDropdownOption(key: string, value: string) {
    if (!hudRoot) return;
    const dropdowns = hudRoot.querySelectorAll<HTMLElement>(
      "[data-control-dropdown]",
    );
    let dropdown: HTMLElement | null = null;
    for (let i = 0; i < dropdowns.length; i++) {
      if (dropdowns[i].dataset.dropdownKey === key) {
        dropdown = dropdowns[i];
        break;
      }
    }
    if (!dropdown) return;
    const input = dropdown.querySelector<HTMLInputElement>(
      "[data-dropdown-input]",
    );
    if (!input) return;
    input.value = value;
    const trigger = dropdown.querySelector<HTMLElement>(
      "[data-role='dropdown-trigger']",
    );
    setHudDropdownOpen(null, trigger);
    input.dispatchEvent(new Event("change", { bubbles: true }));
  }

  function handleHudDropdownKeydown(event: KeyboardEvent): boolean {
    const target = event.target;
    if (
      !(target instanceof HTMLElement) ||
      !hudRoot ||
      !hudRoot.contains(target)
    )
      return false;
    const trigger = target.closest<HTMLButtonElement>(
      "[data-role='dropdown-trigger']",
    );
    const option = target.closest<HTMLButtonElement>(
      "[data-command='select_dropdown']",
    );
    if (trigger) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].indexOf(event.key) < 0)
        return false;
      const key = trigger.dataset.dropdownKey;
      if (!key) return false;
      event.preventDefault();
      setHudDropdownOpen(key);
      const dropdown = trigger.closest<HTMLElement>("[data-control-dropdown]");
      const options = dropdown
        ? Array.from(
            dropdown.querySelectorAll<HTMLButtonElement>(
              "[data-command='select_dropdown']",
            ),
          )
        : [];
      const selected = options.find(
        (entry) => entry.getAttribute("aria-selected") === "true",
      );
      const next =
        event.key === "ArrowUp"
          ? options[options.length - 1]
          : selected || options[0];
      if (next) next.focus({ preventScroll: true });
      return true;
    }
    if (option) {
      const dropdown = option.closest<HTMLElement>("[data-control-dropdown]");
      const options = dropdown
        ? Array.from(
            dropdown.querySelectorAll<HTMLButtonElement>(
              "[data-command='select_dropdown']",
            ),
          )
        : [];
      const index = options.indexOf(option);
      if (event.key === "Escape") {
        event.preventDefault();
        const trigger = dropdown?.querySelector<HTMLElement>(
          "[data-role='dropdown-trigger']",
        );
        setHudDropdownOpen(null, trigger);
        return true;
      }
      if (event.key === "Tab") {
        setHudDropdownOpen(null);
        return false;
      }
      let nextIndex = index;
      if (event.key === "ArrowDown")
        nextIndex = Math.min(options.length - 1, index + 1);
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
    "arrow-up",
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
    sam: "sam-missile",
  };

  const RING_GLYPH_OPEN =
    '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">';

  const HUD_GLYPHS: Record<string, string> = {
    plus:
      '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
      '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
      '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
      '<path d="M7 10l6 0"/>' +
      '<path d="M10 7l0 6"/>' +
      '<path d="M21 21l-6 -6"/>' +
      "</svg>",
    minus:
      '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
      '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
      '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
      '<path d="M7 10l6 0"/>' +
      '<path d="M21 21l-6 -6"/>' +
      "</svg>",
    home:
      '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">' +
      '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
      '<path d="M12.707 2.293l9 9c.63 .63 .184 1.707 -.707 1.707h-1v6a3 3 0 0 1 -3 3h-10a3 3 0 0 1 -3 -3v-6h-1c-.89 0 -1.337 -1.077 -.707 -1.707l9 -9a1 1 0 0 1 1.414 0m.793 8.707h-3a1.5 1.5 0 0 0 -1.5 1.5v3a1.5 1.5 0 0 0 1.5 1.5h3a1.5 1.5 0 0 0 1.5 -1.5v-3a1.5 1.5 0 0 0 -1.5 -1.5"/>' +
      "</svg>",
    zoom_in:
      '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
      '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
      '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
      '<path d="M7 10l6 0"/>' +
      '<path d="M10 7l0 6"/>' +
      '<path d="M21 21l-6 -6"/>' +
      "</svg>",
    zoom_out:
      '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
      '<path stroke="none" d="M0 0h24v24H0z" fill="none"/>' +
      '<path d="M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0"/>' +
      '<path d="M7 10l6 0"/>' +
      '<path d="M21 21l-6 -6"/>' +
      "</svg>",
    close: '<path d="M18 6 6 18M6 6l12 12"/>',
    inbox:
      '<svg class="sow-hud__action-icon" viewBox="0 0 36 39" fill="currentColor" aria-hidden="true">' +
      '<path d="M26.9474 14.6637C27.3732 15.5695 27.9073 16.266 28.5497 16.746C29.1885 17.226 29.7911 17.5183 30.3505 17.6266C30.9099 17.7348 31.4981 18.0416 32.1116 18.5468C32.7251 19.052 33.1906 19.7882 33.5118 20.7482C34.1 22.401 33.1112 24.5518 30.549 27.1935C27.9867 29.8351 24.6269 31.9282 20.4624 33.4764C16.0849 35.0246 12.126 35.6236 8.57499 35.2772C5.02753 34.9271 2.95969 33.9275 2.37146 32.2747C1.83735 30.8348 1.99614 29.3515 2.85143 27.8322C3.70671 26.3129 3.94489 24.8297 3.57319 23.3898C2.07914 18.2653 1.45121 14.2631 1.693 11.3833C1.93479 8.5035 3.43966 5.94125 6.21483 3.69658C6.90772 3.10835 7.29386 2.42989 7.37686 1.654C7.45626 0.881715 7.8424 0.358439 8.5389 0.091388C9.17766 -0.121531 9.79115 0.037256 10.3794 0.571359C10.9676 1.10546 11.7146 1.34364 12.6205 1.29312C16.1426 1.23899 18.7843 2.11953 20.5454 3.93476C22.3029 5.7536 24.4393 9.32631 26.9474 14.6637ZM19.5024 30.8348C21.8518 29.9795 23.9701 28.8463 25.8648 27.4317C27.7594 26.017 29.0946 24.8045 29.8669 23.7904C30.6392 22.7763 30.9748 22.1087 30.8666 21.7875C30.6536 21.1993 30.0004 20.759 28.907 20.4667C27.8135 20.1744 26.1571 20.1599 23.9449 20.427C21.7327 20.694 19.2354 21.3328 16.4602 22.3469C13.7392 23.3609 11.4295 24.523 9.53493 25.8294C7.64031 27.1357 6.38805 28.2581 5.77456 29.1928C5.16106 30.1274 4.93371 30.8348 5.09249 31.3147C5.20076 31.6359 5.86478 31.9282 7.09538 32.1953C8.32237 32.4623 10.1087 32.5165 12.4581 32.3541C14.8074 32.1917 17.1531 31.6864 19.5024 30.8348ZM17.0196 23.8698C17.2325 23.8156 17.514 23.7218 17.8604 23.5883C18.2069 23.4548 18.4342 23.3609 18.5425 23.3068L18.6219 23.3862C18.9972 24.4544 18.7698 25.5623 17.9398 26.7063C17.1134 27.8539 15.9261 28.6947 14.3779 29.2288C11.8157 30.1888 9.78754 30.0011 8.2935 28.6695C10.3758 26.8579 13.2845 25.2556 17.0196 23.8698Z"/>' +
      "</svg>",
    emote:
      '<path d="M20 11.5a7.5 7.5 0 0 1-8 7.5 8.7 8.7 0 0 1-3.5-.7L4 20l1.4-3.6A7 7 0 0 1 4 12c0-4.1 3.6-7.5 8-7.5s8 3.1 8 7Z"/><path d="M9 11h.01M15 11h.01M9 14.5s1 1.5 3 1.5 3-1.5 3-1.5"/>',
    rankings:
      '<svg class="sow-hud__action-icon" viewBox="0 0 48 41" fill="currentColor" aria-hidden="true">' +
      '<path d="M3.43115 14.2054L5.09646 35.0217H15.0883V14.2054H3.43115Z"/>' +
      '<path d="M15.6436 1.71558C16.4762 12.2625 18.1415 33.6895 18.1415 35.0217H31.464L33.1293 1.71558H15.6436Z"/>' +
      '<path d="M44.2317 20.034H33.4072V34.1891H42.5664L44.2317 20.034Z"/>' +
      "</svg>",
    leaderboard:
      '<svg class="sow-hud__action-icon" viewBox="0 0 48 41" fill="currentColor" aria-hidden="true">' +
      '<path d="M3.43115 14.2054L5.09646 35.0217H15.0883V14.2054H3.43115Z"/>' +
      '<path d="M15.6436 1.71558C16.4762 12.2625 18.1415 33.6895 18.1415 35.0217H31.464L33.1293 1.71558H15.6436Z"/>' +
      '<path d="M44.2317 20.034H33.4072V34.1891H42.5664L44.2317 20.034Z"/>' +
      "</svg>",
    cup:
      '<svg class="sow-hud__action-icon" viewBox="0 0 36 34" fill="currentColor" aria-hidden="true">' +
      '<path fill-rule="evenodd" clip-rule="evenodd" d="M28.8428 0H7.44094V2.48287L1.02324 2.48287C0.45812 2.48287 0 2.94099 0 3.50611V14.129C0 18.63 5.02309 22.3338 9.95271 22.7191C11.571 24.453 14.4175 25.7125 16.951 26.2496V29.2936C14.5283 30.0377 10.2008 31.786 9.95271 33.6909H26.5792C26.3287 31.786 21.5603 30.0353 19.1328 29.2936V26.2496C21.6759 25.7125 24.6501 24.453 26.2708 22.7191C31.198 22.3338 36 18.63 36 14.129V3.50611C36 2.94099 35.5419 2.48287 34.9768 2.48287H28.8428V0ZM2.28495 4.68973H7.26755V14.7214C7.26755 16.701 7.77809 18.5842 8.67636 20.2796C4.94844 19.0924 2.28495 16.0339 2.28495 12.4553V4.68973ZM21.8016 16.925L18.0825 14.8756L14.3593 16.925L15.0702 12.5902L12.0599 9.52209L16.2189 8.88873L18.0825 4.94889L19.942 8.88873L24.101 9.52209L21.0927 12.5902L21.8016 16.925ZM27.328 20.5999C28.2311 18.8274 28.7489 16.8672 28.7489 14.7937V4.75441H33.7435V12.4264C33.7435 16.1664 31.068 19.3597 27.328 20.5999Z"/>' +
      "</svg>",
    search: '<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/>',
    options:
      '<svg class="sow-hud__action-icon" viewBox="70 135 860 775" fill="currentColor" aria-hidden="true">' +
      '<path d="M341,205c22.61-2.97,45.75-.43,68,4c5.93-17.06,20.41-35.48,30-51c30.77,6.9,63.06,23.94,88.06,42.94c9.99,7.59,18.86,16.75,28.94,24.06c-2.72,13.76-24.17,40.64-32,54c6.35,9.77,13.98,18.47,19.98,28.53C557.54,330.24,564.77,353.8,572,379c8.03,1.11,55.3,6.03,58.86,9.19c3.64,3.22,3.43,16.65,3.46,21.41c.21,39.07-5.77,74.39-18.32,111.4c-22.08-1.42-44.14-5.63-66-9c-8.44,21.72-26.54,52.27-48,63c1.83,11.12,25.68,53.57,23,60c-.82,1.97-11.84,8.03-14.18,9.49C478.2,664.88,439.36,684.31,400,684c-4.9-11.59-22-44.37-22-55c-23.77,.37-45.47-1.93-69-4c-9.82,17.51-19.87,35.3-31,52c-43.13-8.12-85.06-41.51-115-72c4-10.54,31.11-47.38,31-53c-.04-2.19-17.99-25.01-20.55-29.44C160.3,499.79,151.48,475.94,147,450c-8.18-1.7-58.86-8.15-61.62-10.63c-1.35-1.21-1.87-11.53-2.05-13.79C80.34,387.45,88.66,341.76,109,309c8.84,.39,57.25,10.02,61,7c3.88-3.13,8.56-14.9,11.67-19.63C191.1,282.04,202.55,268.7,216,258c-2.51-8.07-23.64-52.49-22-57c.65-1.79,8.44-6.25,10.43-7.63C231.08,174.97,284.22,147.01,317,148c4.84,11.37,24,47.28,24,57ZM271,323c-15.23,6.47-25.38,27.19-31.26,41.66c-54.92,135.4,112.99,242.21,215.25,137.59c15.77-16.14,26.19-37.45,30.8-59.43C507.44,339.5,400.97,255.7,305.16,298.4C298.57,301.33,271,316.01,271,323Z"/>' +
      '<path d="M549,715c-6.33-29.98-2.8-58.17,7-87c-11.93-9.74-24.22-19.09-36-29c8.75-30.05,40.88-63.94,67-80c8.2,6.7,24.51,23.56,33.74,26.75c.66,.22,43.57-18.73,56.26-20.75c.69-6.78,2.03-40.4,5.31-44.01c12.66-13.95,87.33,.43,103.69,9.01c-1.03,15.87-4,32.17-4,48c5.6,2.84,40.32,26.85,40.59,26.87c7.85,.64,34.03-16.81,42.41-20.87c20.65,19.28,52,60.93,52,90c-14.45,7.44-28.85,14.99-43,23c5.29,27.32,4.64,60.72-8,86c5.2,2.3,35.43,23.89,37,28c4.16,10.88-48.89,79.17-61.73,82.75c-3.33,.93-10.04-5.5-12.53-7.62c-5.73-4.87-19.87-19.01-26.47-20.86c-2.81-.78-10.77,4.98-13.47,6.54C775.83,839.27,760.29,847,745,847c-.36,6.01-3.06,46.71-6,49c-1.36,1.06-9.82,1.43-11.9,1.62c-30.36,2.78-64.84-2.67-92.1-16.62c1.76-15.65,3.69-31.31,5-47c-11.59-3.48-39.29-26.62-42-27c-6.81-.95-32.33,19.93-40.66,21.78c-11.7,2.61-42.65-64.85-45.99-76c-1.18-3.94-5.16-14.14-2.35-17.78c1.6-2.07,38.19-20,40-20Zm237-94c-11.04-14.65-26.67-23.97-44.02-29.4c-97.34-30.43-169.37,78.97-106.65,159.16c9.87,12.61,26.41,22.85,41.28,28.32C772.65,814.43,851.38,698.48,786,621Z"/>' +
      "</svg>",
    leave:
      '<path d="M13 4h6a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-6M11 16l4-4-4-4M15 12H3"/>',
    camera:
      '<circle cx="12" cy="12" r="7"/><circle cx="12" cy="12" r="2"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3"/>',
    tools:
      '<path d="M15 5a4.5 4.5 0 0 0-6 6.2L3.5 16.7a1.8 1.8 0 0 0 2.5 2.5L11.5 13.7A4.5 4.5 0 0 0 17.7 8L15 10.7l-2-2L15.7 6A4.5 4.5 0 0 0 15 5Z"/>',
    retreat:
      RING_GLYPH_OPEN +
      '<path d="M9 3 2 10l7 7v-5h5a4 4 0 0 1 0 8h-3v3h3a7 7 0 0 0 0-14H9Z"/>' +
      "</svg>",
    spawn: '<path d="M5 21V4m1 1h12l-3 4 3 4H6"/>',
    attack:
      '<path d="m5 4 14 16M19 4 5 20M3 6l3-3 3 3M15 18l3 3 3-3M3 18l3 3 3-3M15 6l3-3 3 3"/>',
    fleet:
      '<path d="M3 15h18l-3 5H7l-4-5ZM6 12l6-9 6 9M12 3v12M2 21c2 1 3 1 5 0s3-1 5 0 3 1 5 0 3-1 5 0"/>',
    transfer: '<path d="M4 7h15l-3-3M20 17H5l3 3M19 7l-3 3M5 17l3-3"/>',
    // Filled send glyph: Wikimedia Commons "Send icon.svg" (CC0 1.0, by Lycopene579), recolored to currentColor.
    send:
      '<svg class="sow-hud__action-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">' +
      '<path fill-rule="evenodd" d="M16.6915026,12.4744748 L3.50612381,13.2599618 C3.19218622,13.2599618 3.03521743,13.4170592 3.03521743,13.5741566 L1.15159189,20.0151496 C0.8376543,20.8006365 0.99,21.89 1.77946707,22.52 C2.41,22.99 3.50612381,23.1 4.13399899,22.8429026 L21.714504,14.0454487 C22.6563168,13.5741566 23.1272231,12.6315722 22.9702544,11.6889879 C22.8132856,11.0605983 22.3423792,10.4322088 21.714504,10.118014 L4.13399899,1.16346272 C3.34915502,0.9 2.40734225,1.00636533 1.77946707,1.4776575 C0.994623095,2.10604706 0.8376543,3.0486314 1.15159189,3.99121575 L3.03521743,10.4322088 C3.03521743,10.5893061 3.34915502,10.7464035 3.50612381,10.7464035 L16.6915026,11.5318905 C16.6915026,11.5318905 17.1624089,11.5318905 17.1624089,12.0031827 C17.1624089,12.4744748 16.6915026,12.4744748 16.6915026,12.4744748 Z"/>' +
      "</svg>",
    // Question bubble for requests, drawn as a line glyph like the other actions.
    request:
      '<path d="M5 3h14a3 3 0 0 1 3 3v8a3 3 0 0 1-3 3h-7l-4.5 4v-4H5a3 3 0 0 1-3-3V6a3 3 0 0 1 3-3z"/>' +
      '<path d="M10 7.8a2 2 0 1 1 3 1.7c-.6.3-1 .7-1 1.4v.4M12 13.9h.01"/>',
    city: '<path d="M3 21h18M5 21V10l7-6 7 6v11M9 21v-6h6v6"/>',
    nuke: '<path d="M12 22V9M9 12l3-3 3 3M12 9l5-5M17 4h4M17 4V1M7 21h10M9 18h6"/>',
    upgrade:
      '<path d="m12 3 2.2 5.2L20 10l-4.4 3.5L17 19l-5-3-5 3 1.4-5.5L4 10l5.8-1.8L12 3Z"/>',
    anchor:
      '<path d="M12 3v12M8 7a4 4 0 1 0 8 0M4 13a8 8 0 0 0 16 0M2 13h4M18 13h4"/>',
    factory:
      '<path d="M3 21V9l6 3V8l6 4V5h6v16H3ZM17 8h1M17 11h1M7 17h2M12 17h2M17 17h1"/>',
    farm: '<path d="M12 21V8M12 12c-4-1-6-3-6-6 4 0 6 2 6 6ZM12 15c4-1 6-3 6-6-4 0-6 2-6 6Z"/>',
    defense: '<path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11Z"/>',
    range:
      '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="2"/><path d="M12 2v3M22 12h-3M12 22v-3M2 12h3"/>',
    speed: '<path d="m13 2-3 8h7l-6 12 1-9H5l8-11Z"/>',
    discount:
      '<circle cx="7" cy="7" r="2"/><circle cx="17" cy="17" r="2"/><path d="m19 5-14 14"/>',
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
    construction:
      '<path d="M14.5 6.5a5 5 0 0 0-6.7 6.7L3 18l3 3 4.8-4.8a5 5 0 0 0 6.7-6.7L14 13l-3-3 3.5-3.5Z"/>',
    lock: '<rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 1 1 8 0v3"/>',
    dice: '<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M9 9h.01M15 9h.01M12 12h.01M9 15h.01M15 15h.01"/>',
    globe:
      '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
    solo: '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
    // Filled silhouettes for the command ring.
    ring_attack:
      RING_GLYPH_OPEN +
      '<path fill-opacity=".72" transform="rotate(-45 12 12)" d="M12 1.5l2.5 4v3h-5v-3L12 1.5ZM8 15.5h8V18h-2.5v2h.5v2h-4v-2h.5v-2H8Z"/>' +
      '<path transform="rotate(45 12 12)" d="M12 1.5l2.5 4v8H16V16h-2.5v4h.5v2h-4v-2h.5v-4H8v-2.5h1.5v-8L12 1.5Z"/>' +
      "</svg>",
    ring_fleet:
      RING_GLYPH_OPEN +
      '<path d="M12 2.5v10H4L12 2.5ZM2 14h20l-4 4.5H6L2 14Z"/>' +
      '<path fill-opacity=".72" d="m14 5.5 5.5 7H14Z"/>' +
      '<path d="M4 21c2.7 1.3 5.3-1.3 8 0s5.3-1.3 8 0" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>' +
      "</svg>",
    ring_nuke:
      RING_GLYPH_OPEN +
      '<path d="M16.75 20.23a9.5 9.5 0 0 1-9.5 0L10 15.46a4 4 0 0 0 4 0l2.75 4.77ZM2.5 12a9.5 9.5 0 0 1 4.75-8.23L10 8.54A4 4 0 0 0 8 12H2.5ZM16.75 3.77A9.5 9.5 0 0 1 21.5 12H16a4 4 0 0 0-2-3.46l2.75-4.77Z"/>' +
      '<circle cx="12" cy="12" r="2.4"/>' +
      "</svg>",
    ring_alliance:
      RING_GLYPH_OPEN +
      '<path fill-opacity=".72" d="m1.75 9.75 3.5-5L8 6.75 4.5 12l-2.75-2.25Zm20.5 0-3.5-5L16 6.75 19.5 12l2.75-2.25Z"/>' +
      '<path d="M8.8 7.65h2.1l-1.5 1.5a2.05 2.05 0 0 0 2.9 2.9l1.3-1.3 4.25 4.15-3.05 3.1a2 2 0 0 1-2.8 0L6 12.6l2.8-4.95Z"/>' +
      '<path d="m10.45 10.1 2.2-2.25a1.6 1.6 0 0 1 1.4-.45l1.35.25 3.1 4.75-1.1.95-3.8-3.7-2.05 1.55a.8.8 0 0 1-1.1-1.1Z"/>' +
      "</svg>",
    ring_pending:
      RING_GLYPH_OPEN +
      '<path fill-rule="evenodd" d="M12 2.5a9.5 9.5 0 1 0 0 19 9.5 9.5 0 0 0 0-19ZM11 6h2v5.4l3.6 2.5-1.2 1.7L11 12.5V6Z"/>' +
      "</svg>",
    ring_send:
      RING_GLYPH_OPEN +
      '<path d="m12 2.5 6 6h-4.5v7h-3v-7H6l6-6Z"/>' +
      '<path fill-opacity=".72" d="M3 15h2.5v4h13v-4H21v6.5H3Z"/>' +
      "</svg>",
    ring_request:
      RING_GLYPH_OPEN +
      '<path d="M10.5 2.5h3v7H18l-6 6-6-6h4.5v-7Z"/>' +
      '<path fill-opacity=".72" d="M3 15h2.5v4h13v-4H21v6.5H3Z"/>' +
      "</svg>",
    ring_spawn:
      RING_GLYPH_OPEN +
      '<path d="M5 3h2.5v1h13L17 8.5l3.5 4.5h-13v6H12v2H3v-2h2V3Z"/>' +
      "</svg>",
    ring_build:
      RING_GLYPH_OPEN +
      '<path transform="rotate(35 12 12)" d="M4 5h11l4 3v3h-5l-2-2h-1v11a1.5 1.5 0 0 1-3 0V9H4V5Z"/>' +
      "</svg>",
    ring_upgrade:
      RING_GLYPH_OPEN +
      '<path d="m12 3 8 6v3.5l-8-6-8 6V9l8-6Zm0 9 8 6v3.5l-8-6-8 6V18l8-6Z"/>' +
      "</svg>",
    ring_emote:
      RING_GLYPH_OPEN +
      '<path fill-rule="evenodd" d="M8 3.5h8a6 6 0 0 1 6 6v3a6 6 0 0 1-6 6h-5l-5 3v-3.35a6 6 0 0 1-4-5.65v-3a6 6 0 0 1 6-6ZM8.5 8.25a1.25 1.25 0 1 0 0 2.5 1.25 1.25 0 0 0 0-2.5Zm7 0a1.25 1.25 0 1 0 0 2.5 1.25 1.25 0 0 0 0-2.5Zm-8 4.25h9a4.75 4.75 0 0 1-9 0Z"/>' +
      "</svg>",
    ring_pin:
      RING_GLYPH_OPEN +
      '<path transform="rotate(35 12 12)" d="M7 3h10v2.5h-1.5v5L18 13v2h-4.5v5L12 22l-1.5-2v-5H6v-2l2.5-2.5v-5H7V3Z"/>' +
      "</svg>",
  };

  function transferPresetButtons(resource: string): string {
    return (
      '<div class="sow-hud__transfer-presets">' +
      ["25", "50", "max"]
        .map(function (percent) {
          return (
            '<button class="sow-hud__transfer-preset" type="button" data-command="set_transfer_amount" data-resource="' +
            resource +
            '" data-percent="' +
            percent +
            '">' +
            (percent === "max" ? SOW_t("hud.max") : percent + "%") +
            "</button>"
          );
        })
        .join("") +
      "</div>"
    );
  }

  // Flips a row between Give and Ask; the CTA follows the rows.
  function transferDirButton(resource: string): string {
    return (
      '<button class="sow-hud__transfer-dir" id="sow-hud-transfer-dir-' +
      resource +
      '" type="button" data-command="flip_transfer_row" data-resource="' +
      resource +
      '"></button>'
    );
  }

  type TransferSide = "send" | "request";
  type TransferKind = TransferSide | "trade";
  type TransferResource = "troops" | "gold";
  type TransferOffer = NonNullable<MenuHudPayload["transfer"]>;

  function wholeAmount(value: number): number {
    return Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0;
  }

  // Rows the player flipped away from the panel's side; a fresh panel flips none.
  const transferFlips: Record<string, boolean> = Object.create(null);

  // A row gives on Send and asks on Request, unless the player flipped it.
  function transferRowSide(
    resource: TransferResource,
    offer: TransferOffer,
    own: MenuHudPayload,
  ): TransferSide {
    const side = transferSide(offer, own);
    if (!transferFlips[resource]) return side;
    return side === "send" ? "request" : "send";
  }

  // The one CTA the rows add up to: all Give is Send, all Ask is Request, a mix is a trade.
  function transferKind(
    offer: TransferOffer,
    own: MenuHudPayload,
  ): TransferKind {
    const troops = transferRowSide("troops", offer, own);
    return troops === transferRowSide("gold", offer, own) ? troops : "trade";
  }

  // The side the panel acts on: the radial's pick, else the stronger army leads.
  function transferSide(
    offer: TransferOffer,
    own: MenuHudPayload,
  ): TransferSide {
    if (offer.mode) return offer.mode;
    return Number(own.troops) >= Number(offer.target_troops)
      ? "send"
      : "request";
  }

  // The server's caps for one side: the payer keeps one troop, and a receiver takes only its room.
  function transferCaps(
    side: TransferSide,
    offer: TransferOffer,
    own: MenuHudPayload,
  ) {
    const send = side === "send";
    const room = send
      ? Number(offer.target_max_troops) - Number(offer.target_troops)
      : Number(own.max_troops) - Number(own.troops);
    return {
      troops: wholeAmount(
        Math.min(Number(send ? own.troops : offer.target_troops) - 1, room),
      ),
      gold: wholeAmount(Number(send ? own.gold : offer.target_gold)),
    };
  }

  // A preset is a share of the stock on the side being sized, never more than that side can move.
  function transferPresetAmount(
    resource: "troops" | "gold",
    percent: string,
    offer: TransferOffer,
    own: MenuHudPayload,
  ): number {
    const side = transferRowSide(resource, offer, own);
    const cap = transferCaps(side, offer, own)[resource];
    if (percent === "max") return cap;
    const stock =
      side === "send"
        ? { troops: own.troops, gold: own.gold }
        : { troops: offer.target_troops, gold: offer.target_gold };
    return Math.min(
      cap,
      wholeAmount((Number(stock[resource]) * Number(percent)) / 100),
    );
  }

  // Why a CTA cannot act right now, or null when it is live. A trade needs both of its sides.
  function transferBlockReason(
    kind: TransferKind,
    offer: TransferOffer,
    own: MenuHudPayload,
  ): string | null {
    // A gift names its ally; the all-allies panel only asks or trades.
    if (kind === "send" && offer.all_allies)
      return SOW_t("hud.transfer_all_allies_send");
    const waitMs =
      kind !== "send"
        ? (transferRequestReadyAt[offer.target_id] || 0) - Date.now()
        : 0;
    if (waitMs > 0) return SOW_t("hud.transfer_request_cooldown");
    for (const side of kind === "trade"
      ? (["send", "request"] as const)
      : [kind]) {
      const caps = transferCaps(side, offer, own);
      const stuck = (["troops", "gold"] as const).every(function (resource) {
        return (
          (kind === "trade" &&
            transferRowSide(resource, offer, own) !== side) ||
          caps[resource] === 0
        );
      });
      if (!offer.target_alive || stuck)
        return SOW_t(
          side === "send"
            ? "hud.transfer_blocked_send"
            : "hud.transfer_blocked_request",
        );
    }
    return null;
  }

  function hudIcon(name: string, cls?: string): string {
    const glyph = HUD_GLYPHS[BUILDING_GLYPHS[name] || name];
    if (glyph) {
      if (glyph.startsWith("<svg")) {
        const targetCls = cls || "sow-hud__action-icon";
        return glyph.replace(
          'class="sow-hud__action-icon"',
          'class="' + targetCls + '"',
        );
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
  window.SOW_hudIcon = function (name: string, cls = "sow-menu__inline-icon") {
    return hudIcon(name, cls);
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
      (Math.max(0, STATUS_CELLS.indexOf(kind)) * 100) /
        (STATUS_CELLS.length - 1) +
      "% 0;";
    return (
      '<span class="' +
      (cls || "sow-hud__action-icon") +
      ' sow-hud__status-icon" style="' +
      style +
      '" aria-hidden="true"></span>'
    );
  }

  function formatEndgameExact(
    value: number | string | null | undefined,
  ): string {
    return String(Math.max(0, Math.trunc(Number(value) || 0)));
  }

  function endgameStatsMarkup(prefix: string): string {
    let html =
      '<div class="sow-hud__endgame-stats" id="sow-hud-' + prefix + '-stats">';
    html +=
      '      <div class="sow-hud__endgame-stat" role="group" aria-label="' +
      esc(SOW_t("endgame.stat_kills")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("endgame.stat_kills")) +
      '" data-sow-tooltip-tone="gold"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' +
      statusIcon("death", "sow-hud__inline-icon") +
      '</span><b id="sow-hud-' +
      prefix +
      '-kills">0</b></div>';
    html +=
      '      <div class="sow-hud__endgame-stat" role="group" aria-label="' +
      esc(SOW_t("endgame.stat_assists")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("endgame.stat_assists")) +
      '" data-sow-tooltip-tone="gold"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' +
      hudIcon("ring_alliance", "sow-hud__inline-icon") +
      '</span><b id="sow-hud-' +
      prefix +
      '-assists">0</b></div>';
    html +=
      '      <div class="sow-hud__endgame-stat" role="group" aria-label="' +
      esc(SOW_t("endgame.stat_max_territory")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("endgame.stat_max_territory")) +
      '" data-sow-tooltip-tone="gold"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' +
      '<svg class="sow-hud__inline-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">' +
      '<polygon points="12,2 17,5 17,10.5 12,13.5 7,10.5 7,5" fill="#34d399" fill-opacity="0.28" stroke="#34d399" stroke-width="1.6"/>' +
      '<polygon points="7,10.5 12,13.5 12,19 7,22 2,19 2,13.5" fill="#fbbf24" fill-opacity="0.22" stroke="#fbbf24" stroke-width="1.6"/>' +
      '<polygon points="17,10.5 22,13.5 22,19 17,22 12,19 12,13.5" fill="#34d399" fill-opacity="0.28" stroke="#34d399" stroke-width="1.6"/>' +
      '</svg>' +
      '</span><b id="sow-hud-' +
      prefix +
      '-max-territory">0</b></div>';
    html +=
      '      <div class="sow-hud__endgame-stat" role="group" aria-label="' +
      esc(SOW_t("endgame.stat_gold_received")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("endgame.stat_gold_received")) +
      '" data-sow-tooltip-tone="gold"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' +
      hudIcon("gold", "sow-hud__inline-icon") +
      '</span><b id="sow-hud-' +
      prefix +
      '-gold-received">0</b></div>';
    html +=
      '      <div class="sow-hud__endgame-stat" role="group" aria-label="' +
      esc(SOW_t("endgame.stat_troops_received")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("endgame.stat_troops_received")) +
      '" data-sow-tooltip-tone="gold"><span class="sow-hud__endgame-stat-icon" aria-hidden="true">' +
      hudIcon("ring_attack", "sow-hud__inline-icon") +
      '</span><b id="sow-hud-' +
      prefix +
      '-troops-received">0</b></div>';
    html += "    </div>";
    return html;
  }

  function endgameStatRefs(prefix: string): HudEndgameRefs {
    return {
      kills: document.getElementById("sow-hud-" + prefix + "-kills"),
      assists: document.getElementById("sow-hud-" + prefix + "-assists"),
      maxTerritory: document.getElementById(
        "sow-hud-" + prefix + "-max-territory",
      ),
      goldReceived: document.getElementById(
        "sow-hud-" + prefix + "-gold-received",
      ),
      troopsReceived: document.getElementById(
        "sow-hud-" + prefix + "-troops-received",
      ),
    };
  }

  let endgameRollupActive = false;
  let endgameRollupCancel: (() => void) | null = null;

  function renderEndgameStats(
    refs: HudEndgameRefs,
    stats: HudEndgameStats | null | undefined,
  ): void {
    stats = stats || {};
    if (refs.kills) refs.kills.textContent = formatEndgameExact(stats.kills);
    if (refs.assists)
      refs.assists.textContent = formatEndgameExact(stats.assists);
    if (refs.maxTerritory)
      refs.maxTerritory.textContent = formatEndgameCompact(stats.max_territory);
    if (refs.goldReceived)
      refs.goldReceived.textContent = formatEndgameCompact(stats.gold_received);
    if (refs.troopsReceived)
      refs.troopsReceived.textContent = formatEndgameCompact(
        stats.troops_received,
      );
  }

  function startEndgameStatRollup(
    refs: HudEndgameRefs,
    stats: HudEndgameStats | null | undefined,
  ): void {
    endgameRollupCancel?.();
    const safeStats = stats || {};
    const targets = [
      Number(safeStats.kills) || 0,
      Number(safeStats.assists) || 0,
      Number(safeStats.max_territory) || 0,
      Number(safeStats.gold_received) || 0,
      Number(safeStats.troops_received) || 0,
    ];
    if (typeof window.requestAnimationFrame !== "function" || targets.every((v) => v <= 0)) {
      renderEndgameStats(refs, safeStats);
      return;
    }
    const baseDelayMs = 200;
    const staggerMs = 45;
    const durationMs = 280;
    const totalMs = baseDelayMs + (targets.length - 1) * staggerMs + durationMs;
    const started = performance.now();
    let frameId = 0;
    let stopped = false;
    endgameRollupActive = true;

    const writeValues = function (vals: number[]) {
      if (refs.kills) refs.kills.textContent = formatEndgameExact(vals[0]);
      if (refs.assists) refs.assists.textContent = formatEndgameExact(vals[1]);
      if (refs.maxTerritory) refs.maxTerritory.textContent = formatEndgameCompact(vals[2]);
      if (refs.goldReceived) refs.goldReceived.textContent = formatEndgameCompact(vals[3]);
      if (refs.troopsReceived) refs.troopsReceived.textContent = formatEndgameCompact(vals[4]);
    };

    const stop = function (commitFinal: boolean) {
      if (stopped) return;
      stopped = true;
      endgameRollupActive = false;
      if (frameId) cancelAnimationFrame(frameId);
      if (commitFinal) renderEndgameStats(refs, safeStats);
      if (endgameRollupCancel === cancelFn) endgameRollupCancel = null;
    };
    const cancelFn = function () {
      stop(false);
    };
    endgameRollupCancel = cancelFn;

    const frame = function (now: number) {
      if (stopped) return;
      const elapsed = now - started;
      if (elapsed >= totalMs) {
        stop(true);
        return;
      }
      writeValues(computeEndgameStatRollups(elapsed, targets, baseDelayMs, staggerMs, durationMs));
      frameId = requestAnimationFrame(frame);
    };
    writeValues([0, 0, 0, 0, 0]);
    frameId = requestAnimationFrame(frame);
  }

  function activateDeferredBuildingAtlases() {
    if (!hudRoot) return;
    hudRoot
      .querySelectorAll<HTMLElement>("[data-sow-building-atlas]")
      .forEach((icon) => {
        const atlas = icon.dataset.sowBuildingAtlas;
        if (!atlas) return;
        icon.style.backgroundImage = 'url("' + atlas + '")';
        icon.removeAttribute("data-sow-building-atlas");
      });
  }

  function buildingIcon(kind: string, level?: number, cls?: string): string {
    const name = String(kind || "");
    if (name === "MissileSilo")
      return unitIcon("atom-bomb", cls || "sow-hud__building-icon");
    if (name.toLowerCase() === "sam")
      return unitIcon("sam-missile", cls || "sow-hud__building-icon");
    const art = window.SOW_BUILDING_ART;
    const canonical =
      name.charAt(0).toUpperCase() + name.slice(1).toLowerCase();
    const atlas = art && art.sprites && art.sprites[canonical];
    const sprite = atlas && atlas[String(Number(level) || 1)];
    if (
      sprite &&
      art.cell_size > 0 &&
      art.atlas_width > art.cell_size &&
      art.atlas_height > art.cell_size
    ) {
      const backgroundWidth = ((art.atlas_width * 100) / art.cell_size).toFixed(
        4,
      );
      const backgroundHeight = (
        (art.atlas_height * 100) /
        art.cell_size
      ).toFixed(4);
      const positionX = (
        (sprite.x * 100) /
        (art.atlas_width - art.cell_size)
      ).toFixed(4);
      const positionY = (
        (sprite.y * 100) /
        (art.atlas_height - art.cell_size)
      ).toFixed(4);
      const atlasUrl = escapeHudText(asset(art.atlas));
      const ready = window.SOW_INITIAL_LOADER_READY === true;
      const style =
        (ready ? "background-image:url(&quot;" + atlasUrl + "&quot;);" : "") +
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
        ' is-pixel-art"' +
        (ready ? "" : ' data-sow-building-atlas="' + atlasUrl + '"') +
        ' style="' +
        style +
        '" aria-hidden="true"></span>'
      );
    }
    const glyph = BUILDING_GLYPHS[name.toLowerCase()];
    return glyph ? hudIcon(glyph, cls || "sow-hud__building-icon") : "";
  }

  // Ships and nukes use the same pixel-art path as buildings, at the same size.
  function unitIcon(name: string, cls?: string): string {
    return (
      '<span class="' +
      (cls || "sow-hud__building-icon") +
      ' is-pixel-art" style="background-image:url(&quot;' +
      escapeHudText(asset("gameplay/units/" + name + ".webp")) +
      '&quot;);background-size:contain;" aria-hidden="true"></span>'
    );
  }

  function structureOrUnitIcon(
    kind: string,
    level?: number,
    cls?: string,
  ): string {
    if (kind === "Warship") return unitIcon("warship", cls);
    if (kind === "Nuke") return unitIcon("atom-bomb", cls);
    if (kind === "AntiNuke") return unitIcon("sam-missile", cls);
    return buildingIcon(kind, level, cls);
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

  function dockHotkey(hotkey: string | number): string {
    return '<kbd class="sow-hud__dock-hotkey" aria-hidden="true">' + hotkey + "</kbd>";
  }

  function leaderById(id: string | null | undefined): MenuLeader {
    const leaders =
      hudState && Array.isArray(hudState.leaders) ? hudState.leaders : [];
    const normalize = function (value: unknown) {
      return String(value || "")
        .replace(/[^a-z0-9]/gi, "")
        .toLowerCase();
    };
    const wanted = normalize(id);
    const found = leaders.find(function (leader) {
      return (
        normalize(leader.id) === wanted ||
        normalize(leader.slug) === wanted ||
        normalize(leader.art_slug) === wanted
      );
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

  function formatNameplateTroops(
    value: number | string | null | undefined,
  ): string {
    let count = Number(value);
    if (!Number.isFinite(count)) return "0";
    const rounded = Math.round(count);
    if (Math.abs(count - rounded) < 1e-3) count = rounded;
    count = Math.max(0, count);
    if (count >= 10000000)
      return (Math.floor(count / 100000) / 10).toFixed(1) + "M";
    if (count >= 1000000)
      return (Math.floor(count / 10000) / 100).toFixed(2) + "M";
    if (count >= 100000) return Math.floor(count / 1000) + "K";
    if (count >= 10000) return (Math.floor(count / 100) / 10).toFixed(1) + "K";
    if (count >= 1000) return (Math.floor(count / 10) / 100).toFixed(2) + "K";
    return String(Math.round(count));
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
    | {
        source: "objective" | "nameplate";
        left: number;
        right: number;
        top: number;
      }
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
        return {
          source: "nameplate",
          left: rect.left,
          right: rect.right,
          top: rect.bottom + gap,
        };
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
    const objective =
      story && story.querySelector<HTMLElement>(".sow-story__objective");
    const nameplate = hudRefs && hudRefs.nameplate;
    if (
      story === notificationLayoutStory &&
      objective === notificationLayoutObjective &&
      nameplate === notificationLayoutNameplate
    )
      return;
    if (
      objective !== notificationLayoutObjective &&
      notificationResizeObserver
    ) {
      if (notificationLayoutObjective)
        notificationResizeObserver.unobserve(notificationLayoutObjective);
      if (objective) notificationResizeObserver.observe(objective);
    }
    if (
      nameplate !== notificationLayoutNameplate &&
      notificationResizeObserver
    ) {
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
    if (
      !notificationResizeObserver &&
      typeof window.ResizeObserver === "function"
    ) {
      notificationResizeObserver = new window.ResizeObserver(function () {
        syncCompactHudBounds();
        syncNotificationPlacement();
      });
    }
    if (
      !notificationMutationObserver &&
      typeof window.MutationObserver === "function"
    ) {
      notificationMutationObserver = new window.MutationObserver(function () {
        observeNotificationAnchors();
        syncCompactHudDensity();
        scheduleNotificationPlacement();
      });
    }
    [
      hudRefs.topbar,
      hudRefs.nameplate,
      hudRefs.dock,
      hudRefs.dockStack,
    ].forEach(function (element) {
      if (element && notificationResizeObserver)
        notificationResizeObserver.observe(element);
    });
    observeNotificationAnchors();
    syncCompactHudBounds();
    if (!notificationLayoutStarted) {
      notificationLayoutStarted = true;
      const onViewportChange = function () {
        const hudScreen = measureHudScreen();
        syncCompactHudBounds(hudScreen);
        if (hudState && hudState.phase === "Playing" && hudState.hud) {
          const mapMenu = hudState.hud.map_menu;
          if (mapMenu && mapMenu.open && mapMenu.view === "building_details")
            renderMapMenu(mapMenu);
          renderPlacementPanel(hudState.hud.building_placement);
        }
        renderNotifications([], false, hudScreen);
        if (
          hudRefs &&
          (hudRefs.requestToastCards.length > 0 ||
            requestToastDeck.entries.length > 0)
        ) {
          renderRequestToasts(
            latestInboxRequests,
            activeHudPanel === "inbox",
            hudScreen,
          );
        }
        scheduleNotificationPlacement();
      };
      window.addEventListener("resize", onViewportChange, { passive: true });
      if (window.visualViewport) {
        window.visualViewport.addEventListener("resize", onViewportChange, {
          passive: true,
        });
        window.visualViewport.addEventListener("scroll", onViewportChange, {
          passive: true,
        });
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
      window.addEventListener(
        "sow:locale-change",
        scheduleNotificationPlacement,
      );
    }
    scheduleNotificationPlacement();
  }

  function syncNotificationPlacement() {
    if (!hudRefs || !hudRefs.notifications) return;
    const notifications = hudRefs.notifications;
    const story = document.getElementById("sow-story");
    const objective =
      story && story.querySelector<HTMLElement>(".sow-story__objective");
    const anchor = notificationAnchor(story, objective, hudRefs.nameplate, 12);
    let inlineStart = null;
    if (anchor.source !== "safe") {
      const direction = window.getComputedStyle
        ? window.getComputedStyle(hudRoot!).direction
        : document.documentElement.dir;
      inlineStart =
        direction === "rtl" ? window.innerWidth - anchor.right : anchor.left;
      const fallbackWidth = Math.min(
        window.innerWidth <= 720 ? 300 : 360,
        Math.max(0, window.innerWidth - 32),
      );
      const notificationWidth = notifications.offsetWidth || fallbackWidth;
      inlineStart = Math.max(
        8,
        Math.min(inlineStart, window.innerWidth - notificationWidth - 8),
      );
    }
    const topValue =
      anchor.source === "safe" ? null : Math.round(Math.max(0, anchor.top));
    const inlineStartValue =
      inlineStart == null ? null : Math.round(inlineStart);
    const state = [anchor.source, topValue, inlineStartValue].join(":");
    if (state === notificationLayoutState) return;
    notificationLayoutState = state;
    if (topValue == null)
      notifications.style.removeProperty("--sow-hud-notifications-top");
    else
      notifications.style.setProperty(
        "--sow-hud-notifications-top",
        topValue + "px",
      );
    if (inlineStartValue == null)
      notifications.style.removeProperty(
        "--sow-hud-notifications-inline-start",
      );
    else
      notifications.style.setProperty(
        "--sow-hud-notifications-inline-start",
        inlineStartValue + "px",
      );
    notifications.classList.toggle("is-positioned", inlineStartValue != null);
    const hasVisibleNotification =
      notificationCards.some(function (parts) {
        return parts && parts.card && !parts.card.hidden;
      }) || leavingNotificationCards.size > 0;
    if (
      hasVisibleNotification &&
      !hudRoot!.classList.contains("is-reduced-motion")
    ) {
      if (!notifications.classList.contains("is-reanchoring")) {
        notifications.classList.add("is-reanchoring");
      }
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
      platePendingTroops: requiredHudElement(
        "sow-hud-nameplate-pending-troops",
      ),
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
      deployFill: requiredHudElement("sow-hud-deploy-fill"),
      deployStatus: requiredHudElement("sow-hud-deploy-status"),
      awaitingPlayers: requiredHudElement("sow-hud-awaiting-players"),
      buildingsStrip: requiredHudElement("sow-hud-buildings-strip"),
      worldProgressOverlay: requiredHudElement(
        "sow-hud-world-progress-overlay",
      ),
      worldCelebrationsOverlay: requiredHudElement(
        "sow-hud-world-celebrations-overlay",
      ),
      buildingButtons: Array.from(
        root.querySelectorAll<HTMLButtonElement>(
          "[data-command='select_building']",
        ),
      ),
      panelButtons: Array.from(
        root.querySelectorAll<HTMLButtonElement>(
          ".sow-hud__topbar [data-command^='toggle_']",
        ),
      ),
      leaderboard: requiredHudElement("sow-hud-leaderboard"),
      rows: requiredHudElement("sow-hud-lb-rows"),
      lbSearch: requiredHudInput("sow-hud-lb-search"),
      lbEmpty: requiredHudElement("sow-hud-lb-empty"),
      lbToolbar: requiredHudElement("sow-hud-lb-toolbar"),
      lbTeamWrap: requiredHudElement("sow-hud-lb-team-wrap"),
      inbox: requiredHudElement("sow-hud-inbox"),
      inboxRows: requiredHudElement("sow-hud-inbox-rows"),
      inboxFilters: requiredHudElement("sow-hud-inbox-filters"),
      inboxFilterButtons: Array.from(
        root.querySelectorAll<HTMLButtonElement>("[data-inbox-filter]"),
      ),
      inboxBulk: requiredHudElement("sow-hud-inbox-bulk"),
      inboxAcceptAll: requiredHudButton("sow-hud-inbox-accept-all"),
      inboxRejectAll: requiredHudButton("sow-hud-inbox-reject-all"),
      inboxClose: requiredHudElement("sow-hud-inbox-close"),
      inboxButton: requiredHudQuery(root, ".sow-hud__inbox-pill"),
      inboxTotal: requiredHudElement("sow-hud-inbox-total"),
      inboxEmpty: requiredHudElement("sow-hud-inbox-empty"),
      inboxRequests: Object.create(null),
      inboxRequestKeys: [],
      inboxRenderKey: "",
      requestToasts: document.createElement("div"),
      requestToastStack: document.createElement("div"),
      requestToastMore: document.createElement("button"),
      requestToastCards: [],
      transfer: requiredHudElement("sow-hud-transfer"),
      transferTitle: requiredHudElement("sow-hud-transfer-title"),
      transferTarget: requiredHudElement("sow-hud-transfer-target"),
      transferClose: requiredHudElement("sow-hud-transfer-close"),
      transferReturnFocus: null,
      transferGold: requiredHudInput("sow-hud-transfer-gold"),
      transferTroops: requiredHudInput("sow-hud-transfer-troops"),
      transferHint: requiredHudElement("sow-hud-transfer-hint"),
      transferSend: requiredHudButton("sow-hud-transfer-send"),
      transferRequest: requiredHudButton("sow-hud-transfer-request"),
      transferTrade: requiredHudButton("sow-hud-transfer-trade"),
      transferDirTroops: requiredHudButton("sow-hud-transfer-dir-troops"),
      transferDirGold: requiredHudButton("sow-hud-transfer-dir-gold"),
      transferStockTroops: requiredHudElement("sow-hud-transfer-stock-troops"),
      transferStockMaxTroops: requiredHudElement(
        "sow-hud-transfer-stock-max-troops",
      ),
      transferStockGold: requiredHudElement("sow-hud-transfer-stock-gold"),
      transferIdentity: createPlayerIdentity(),
      betrayal: requiredHudElement("sow-hud-betrayal-modal"),
      betrayalTitle: requiredHudElement("sow-hud-betrayal-title"),
      betrayalCopy: requiredHudElement("sow-hud-betrayal-copy"),
      betrayalTarget: requiredHudElement("sow-hud-betrayal-target"),
      betrayalCancel: requiredHudElement("sow-hud-betrayal-cancel"),
      betrayalConfirmLabel: requiredHudElement("sow-hud-betrayal-confirm-label"),
      betrayalReturnFocus: null,
      betrayalIdentity: createPlayerIdentity(),
      surrender: requiredHudElement("sow-hud-surrender-modal"),
      surrenderTitle: requiredHudElement("sow-hud-surrender-title"),
      surrenderStats: requiredHudElement("sow-hud-surrender-stats"),
      surrenderStatValues: endgameStatRefs("surrender"),
      surrenderPortrait: requiredHudImage("sow-hud-surrender-portrait"),
      surrenderCancel: requiredHudElement("sow-hud-surrender-cancel"),
      surrenderActionLabel: requiredHudElement(
        "sow-hud-surrender-action-label",
      ),
      endgame: requiredHudElement("sow-hud-endgame-modal"),
      endgameCard: requiredHudQuery(
        document,
        "#sow-hud-endgame-modal .sow-hud__endgame-card",
      ),
      endgameBanner: requiredHudElement("sow-hud-endgame-banner"),
      endgamePortrait: requiredHudImage("sow-hud-endgame-portrait"),
      endgameStatValues: endgameStatRefs("endgame"),
      endgameObserve: requiredHudButton("sow-hud-endgame-observe"),
      mapMenu: requiredHudElement("sow-hud-map-menu"),
      placementOverlay: requiredHudElement("sow-hud-placement-overlay"),
      placementPanel: requiredHudElement("sow-hud-placement-panel"),
      placementArrows: {
        up: requiredHudButtonQuery(
          root,
          "#sow-hud-placement-overlay [data-placement-nudge='up']",
        ),
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

  function compareLeaderboardPlayers(
    a: HudLeaderboardPlayer,
    b: HudLeaderboardPlayer,
    mode: LeaderboardSortMode,
    is1v1: boolean,
  ): number {
    if (is1v1) {
      const aHuman = !a.is_bot;
      const bHuman = !b.is_bot;
      if (aHuman !== bHuman) {
        return aHuman ? -1 : 1;
      }
    }
    if (mode === "troops") {
      const diff = (b.troops || 0) - (a.troops || 0);
      if (diff !== 0) return diff;
    } else if (mode === "gold") {
      const diff = (b.gold || 0) - (a.gold || 0);
      if (diff !== 0) return diff;
    } else if (mode === "troop_rate") {
      const diff = (b.troop_rate || 0) - (a.troop_rate || 0);
      if (diff !== 0) return diff;
    } else if (mode === "gold_rate") {
      const diff = (b.gold_rate || 0) - (a.gold_rate || 0);
      if (diff !== 0) return diff;
    } else if (mode === "kills") {
      const diff = (b.kills || 0) - (a.kills || 0);
      if (diff !== 0) return diff;
    } else if (mode === "alpha") {
      const diff = (a.name || "").localeCompare(b.name || "");
      if (diff !== 0) return diff;
    } else {
      const rankA = Number.isFinite(Number(a.rank)) ? Number(a.rank) : 9999;
      const rankB = Number.isFinite(Number(b.rank)) ? Number(b.rank) : 9999;
      const diff = rankA - rankB;
      if (diff !== 0) return diff;
    }
    const terrDiff = (b.territory_pct || 0) - (a.territory_pct || 0);
    if (terrDiff !== 0) return terrDiff;
    return (b.troops || 0) - (a.troops || 0);
  }

  function renderLeaderboardToolbar(): string {
    const teamDropdown = SOW_renderDropdown({
      key: "lb-team",
      name: "lb_team",
      label: SOW_t("hud.filter_team") || "Team",
      icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11Z"/></svg>',
      value: leaderboardTeamFilter,
      options: [
        {
          value: "all",
          label: SOW_t("hud.filter_all_teams") || "All Teams",
          shortLabel: SOW_t("hud.filter_team") || "Team",
        },
        {
          value: "Red",
          label: SOW_t("hud.filter_red_team") || "Red Team",
        },
        {
          value: "Blue",
          label: SOW_t("hud.filter_blue_team") || "Blue Team",
        },
      ],
    });

    const typeDropdown = SOW_renderDropdown({
      key: "lb-type",
      name: "lb_type",
      label: SOW_t("hud.filter_type") || "Players",
      icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>',
      value: leaderboardTypeFilter,
      options: [
        {
          value: "all",
          label: SOW_t("hud.filter_all_types") || "All",
          shortLabel: SOW_t("hud.filter_type") || "Players",
        },
        {
          value: "humans",
          label: SOW_t("hud.filter_humans") || "Humans",
        },
        {
          value: "bots",
          label: SOW_t("hud.filter_bots") || "Bots",
        },
      ],
    });

    const sortDropdown = SOW_renderDropdown({
      key: "lb-sort",
      name: "lb_sort",
      label: SOW_t("hud.filter_sort") || "Sort",
      icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 16 4 4 4-4"/><path d="M7 20V4"/><path d="m21 8-4-4-4 4"/><path d="M17 4v16"/></svg>',
      value: leaderboardSortMode,
      options: [
        {
          value: "rank",
          label: SOW_t("hud.sort_rank") || "Rank",
          shortLabel: SOW_t("hud.filter_sort") || "Sort",
        },
        {
          value: "troops",
          label: SOW_t("hud.sort_troops") || "Troops",
        },
        {
          value: "gold",
          label: SOW_t("hud.sort_gold") || "Gold",
        },
        {
          value: "troop_rate",
          label: SOW_t("hud.sort_troop_rate") || "Troops/s",
        },
        {
          value: "gold_rate",
          label: SOW_t("hud.sort_gold_rate") || "Gold/s",
        },
        {
          value: "kills",
          label: SOW_t("hud.sort_kills") || "Kills",
        },
        {
          value: "alpha",
          label: SOW_t("hud.sort_alpha") || "A-Z",
        },
      ],
    });

    return (
      '<div class="sow-hud__leaderboard-toolbar" id="sow-hud-lb-toolbar">' +
      '  <div class="sow-hud__lb-filter-item sow-hud__lb-filter-team" id="sow-hud-lb-team-wrap" hidden>' +
      teamDropdown +
      "  </div>" +
      '  <div class="sow-hud__lb-filter-item sow-hud__lb-filter-type">' +
      typeDropdown +
      "  </div>" +
      '  <div class="sow-hud__lb-filter-item sow-hud__lb-filter-sort">' +
      sortDropdown +
      "  </div>" +
      "</div>"
    );
  }

  function ensureHudDom() {
    if (!hudRoot || hudInitialized) return;
    hudInitialized = true;
    hudRoot.toggleAttribute("data-dock-hotkeys", browserZoomInputMode() !== "pinch");
    emoteTextKey = "";
    const initialMusicVol =
      hudState && hudState.settings && hudState.settings.music_volume != null
        ? Number(hudState.settings.music_volume)
        : 0.8;
    const initialMusicPct = Math.round(
      Math.max(0, Math.min(1, initialMusicVol)) * 100,
    );
    const initialSfxVol =
      hudState && hudState.settings && hudState.settings.sfx_volume != null
        ? Number(hudState.settings.sfx_volume)
        : 0.8;
    const initialSfxPct = Math.round(
      Math.max(0, Math.min(1, initialSfxVol)) * 100,
    );
    hudRoot.innerHTML =
      "" +
      '<header class="sow-hud__topbar">' +
      '<div class="sow-hud__match-status">' +
      '<div class="sow-hud__territory-score" id="sow-hud-territory-score" hidden>' +
      '<div class="sow-hud__territory-side" data-side="Red" role="group"><b></b><i aria-hidden="true"></i></div>' +
      '<div class="sow-hud__territory-side" data-side="Blue" role="group"><b></b><i aria-hidden="true"></i></div>' +
      '</div><span class="sow-hud__match-clock" id="sow-hud-match-clock" hidden></span>' +
      '<span class="sow-hud__awaiting-players" id="sow-hud-awaiting-players" role="status" aria-live="polite" hidden>' +
      SOW_t("hud.awaiting_players") +
      "</span></div>" +
      '  <div class="sow-hud__status-right">' +
      '    <button class="sow-hud__icon-btn sow-hud__exit-pill" type="button" data-command="prompt_surrender" aria-label="' +
      esc(SOW_t("hud.leave")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("hud.leave")) +
      '" data-sow-tooltip-placement="bottom" data-sow-tooltip-tone="danger">' +
      hudIcon("leave", "sow-hud__action-icon") +
      "</button>" +
      '    <button class="sow-hud__icon-btn" type="button" data-command="toggle_settings" aria-label="' +
      esc(SOW_t("menu.settings")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("menu.settings")) +
      '" data-sow-tooltip-placement="bottom">' +
      hudIcon("options", "sow-hud__action-icon") +
      "</button>" +
      '    <button class="sow-hud__icon-btn sow-hud__inbox-pill" type="button" data-command="toggle_inbox" aria-label="' +
      esc(SOW_t("hud.inbox")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("hud.inbox")) +
      '" data-sow-tooltip-placement="bottom">' +
      hudIcon("inbox") +
      '<span class="sow-hud__inbox-badge" id="sow-hud-inbox-count" hidden>0</span></button>' +
      '    <button class="sow-hud__icon-btn" type="button" data-command="toggle_leaderboard" aria-label="' +
      esc(SOW_t("hud.rankings")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("hud.rankings")) +
      '" data-sow-tooltip-placement="bottom" data-sow-tooltip-tone="gold">' +
      hudIcon("rankings", "sow-hud__action-icon") +
      "</button>" +
      '    <button class="sow-hud__icon-btn hidden" type="button" data-command="toggle_dev_sidebar" id="sow-hud-dev-btn" hidden aria-label="' +
      esc(SOW_t("hud.dev_tools")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("hud.dev_tools")) +
      '" data-sow-tooltip-placement="bottom">' +
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
      '  <div class="sow-hud__nameplate-avatar-wrap"><img class="sow-hud__nameplate-avatar" id="sow-hud-nameplate-avatar" alt="" draggable="false"></div>' +
      '  <div class="sow-hud__nameplate-copy"><strong id="sow-hud-nameplate-name"></strong>' +
      '  <div class="sow-hud__nameplate-bar"><i id="sow-hud-nameplate-fill"></i><i id="sow-hud-nameplate-pending-troops" class="sow-hud__nameplate-pending" aria-hidden="true"></i><span class="sow-hud__nameplate-readout"><span id="sow-hud-nameplate-troops"></span>' +
      hudIcon("troops", "sow-hud__inline-icon") +
      "</span></div>" +
      '  <div class="sow-hud__nameplate-economy">' +
      '    <span class="sow-hud__nameplate-stat sow-hud__nameplate-stat--gold" data-sow-tooltip="' +
      esc(SOW_t("hud.gold_treasury")) +
      '" data-sow-tooltip-tone="gold" aria-label="' +
      esc(SOW_t("hud.gold_treasury")) +
      '"><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' +
      currencyAsset("gold") +
      '" alt="" aria-hidden="true"><b id="sow-hud-nameplate-gold">0</b></span>' +
      '    <span class="sow-hud__nameplate-stat sow-hud__nameplate-stat--gold-rate" data-sow-tooltip="' +
      esc(SOW_t("hud.gold_net_rate")) +
      '" data-sow-tooltip-tone="gold" aria-label="' +
      esc(SOW_t("hud.gold_net_rate")) +
      '"><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' +
      currencyAsset("gold") +
      '" alt="" aria-hidden="true"><b id="sow-hud-nameplate-gold-rate">0/s</b></span>' +
      '    <span class="sow-hud__nameplate-stat sow-hud__nameplate-stat--troop-rate" data-sow-tooltip="' +
      esc(SOW_t("hud.troop_production_rate")) +
      '" aria-label="' +
      esc(SOW_t("hud.troop_production_rate")) +
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
      '<svg class="sow-hud__inline-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">' +
      '<polygon points="12,2 17,5 17,10.5 12,13.5 7,10.5 7,5" fill="#34d399" fill-opacity="0.28" stroke="#34d399" stroke-width="1.6"/>' +
      '<polygon points="7,10.5 12,13.5 12,19 7,22 2,19 2,13.5" fill="#fbbf24" fill-opacity="0.22" stroke="#fbbf24" stroke-width="1.6"/>' +
      '<polygon points="17,10.5 22,13.5 22,19 17,22 12,19 12,13.5" fill="#34d399" fill-opacity="0.28" stroke="#34d399" stroke-width="1.6"/>' +
      '</svg>' +
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
      '      <input type="range" min="5" max="100" value="' +
      String(DEFAULT_ATTACK_RATIO * 100) +
      '" step="1" class="sow-hud__range-vertical" id="sow-hud-slider" aria-label="' +
      SOW_t("hud.map_action_attack") +
      '">' +
      "    </div>" +
      '    <div class="sow-hud__army-allocation">' +
      '      <strong id="sow-hud-army-ratio" aria-hidden="true">' +
      String(DEFAULT_ATTACK_RATIO * 100) +
      "%</strong>" +
      "    </div>" +
      "  </div>" +
      '  <output class="sow-hud__allocation-detail" id="sow-hud-army-allocation" for="sow-hud-slider" aria-hidden="true"></output>' +
      "</aside>" +
      '<aside class="sow-hud__right-rail" id="sow-hud-right-rail">' +
      '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_in" aria-label="' +
      esc(SOW_t("hud.zoom_in")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("hud.zoom_in")) +
      '" data-sow-tooltip-placement="start">' +
      hudIcon("plus") +
      "</button>" +
      '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_out" aria-label="' +
      esc(SOW_t("hud.zoom_out")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("hud.zoom_out")) +
      '" data-sow-tooltip-placement="start">' +
      hudIcon("minus") +
      "</button>" +
      '  <button type="button" class="sow-hud__icon-btn" data-command="center_camera" aria-label="' +
      esc(SOW_t("hud.center_camera")) +
      '" data-sow-tooltip="' +
      esc(SOW_t("hud.center_camera")) +
      '" data-sow-tooltip-placement="start">' +
      hudIcon("home", "sow-hud__action-icon") +
      "</button>" +
      "</aside>" +
      '<aside class="sow-hud__sheet sow-hud__dev-sidebar hidden" id="sow-hud-dev-sidebar">' +
      '  <div class="sow-hud__sheet-header sow-hud__dev-header">' +
      '    <div class="sow-hud__sheet-title-group">' +
      '      <span class="sow-hud__sheet-title-badge" aria-hidden="true">' +
      hudIcon("tools", "sow-hud__sheet-title-icon") +
      "      </span>" +
      '      <h3 class="sow-hud__sheet-title">' +
      SOW_t("hud.dev_tools") +
      "      </h3>" +
      "    </div>" +
      '    <button type="button" class="sow-hud__sheet-btn sow-hud__dev-save-btn" data-command="dev_save_settings" aria-label="' +
      esc(SOW_t("hud.save_settings") || "Descargar ajustes JSON") +
      '" data-sow-tooltip="' +
      esc(SOW_t("hud.save_settings") || "Descargar ajustes JSON") +
      '">' +
      DEV_SAVE_ICON +
      "</button>" +
      '    <button type="button" class="sow-hud__sheet-btn sow-hud__dev-more-btn" data-command="dev_menu" aria-label="More" aria-expanded="false">⋯</button>' +
      '    <button class="sow-hud__close-btn sow-hud__sheet-btn" type="button" data-command="toggle_dev_sidebar" aria-label="' +
      SOW_t("hud.close_developer_tools") +
      '">' +
      hudIcon("close", "sow-hud__close-icon") +
      "    </button>" +
      '    <div class="sow-hud__dev-menu" id="sow-hud-dev-menu" hidden>' +
      '      <button type="button" data-command="dev_copy_settings">Copiar ajustes JSON</button>' +
      '      <button type="button" data-command="dev_vfx_all" data-on="1">All effects on</button>' +
      '      <button type="button" data-command="dev_vfx_all" data-on="0">All effects off</button>' +
      '      <button type="button" data-command="dev_reset_all" id="sow-hud-dev-reset-all">Reset all</button>' +
      "    </div>" +
      "  </div>" +
      '  <div class="sow-hud__dev-category-bar">' +
      '    <label class="sow-hud__dev-category-label" for="sow-hud-dev-category-select">' +
      '      <span class="sow-hud__dev-category-title">' +
      (SOW_t("hud.category") || "Category") +
      "</span>" +
      '      <i class="sow-hud__dev-category-dot" id="sow-hud-dev-category-dot" hidden></i>' +
      "    </label>" +
      '    <div class="sow-hud__dev-category-field">' +
      '      <select class="sow-hud__dev-category-select" id="sow-hud-dev-category-select" data-dev-category-select aria-label="' +
      (SOW_t("hud.category") || "Category") +
      '"></select>' +
      '      <svg class="sow-hud__dev-category-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" aria-hidden="true"><polyline points="6 9 12 15 18 9"/></svg>' +
      "    </div>" +
      "  </div>" +
      '  <div class="sow-hud__dev-transport" id="sow-hud-dev-transport" hidden>' +
      '    <input type="range" min="0" max="1" step="0.01" data-dev-preview="phase" aria-label="Preview position">' +
      '    <label class="sow-hud__dev-chip"><input type="checkbox" data-dev-preview="paused"><span>Pause</span></label>' +
      '    <label class="sow-hud__dev-chip"><input type="checkbox" data-dev-preview="repeat"><span>Loop</span></label>' +
      "  </div>" +
      '  <div class="sow-hud__dev-search-bar">' +
      hudIcon("search", "sow-hud__dev-search-icon") +
      '    <input class="sow-hud__dev-search sow-control-search__input" type="search" data-dev-search placeholder="Search" aria-label="Find shader setting">' +
      "  </div>" +
      '  <div class="sow-hud__sheet-body sow-hud__dev-body">' +
      '    <div class="sow-hud__dev-perf" id="sow-hud-dev-perf" role="group" aria-label="Frame cost switches" hidden></div>' +
      '    <div class="sow-hud__dev-controls" id="sow-hud-dev-controls"></div>' +
      "  </div>" +
      "</aside>" +
      '<aside class="sow-hud__panel sow-hud__settings sow-hud__sheet hidden" id="sow-hud-settings">' +
      '  <div class="sow-hud__sheet-header">' +
      '    <div class="sow-hud__sheet-title-group">' +
      '      <span class="sow-hud__sheet-title-badge" aria-hidden="true">' +
      hudIcon("options", "sow-hud__sheet-title-icon") +
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
      '    <section class="sow-menu__settings-section sow-hud__settings-section">' +
      '      <label class="sow-hud__setting-row sow-menu__settings-switch-row"><span class="sow-menu__settings-row-label">' +
      SOW_t("hud.sound") +
      '</span><input class="sow-hud__switch" type="checkbox" role="switch" data-hud-setting="mute_all"></label>' +
      '      <label class="sow-hud__setting-row sow-hud__setting-row--slider sow-hud__setting-card sow-hud__setting-card--slider">' +
      '        <div class="sow-menu__slider-label"><span class="sow-menu__settings-row-label">' +
      SOW_t("hud.music") +
      '</span><b id="sow-hud-music-value">' +
      initialMusicPct +
      "%</b></div>" +
      '        <input type="range" min="0" max="1" step="0.01" value="' +
      initialMusicVol +
      '" data-hud-setting="music_volume" style="--sow-slider-fill: ' +
      initialMusicPct +
      "%; --sow-music-fill: " +
      initialMusicPct +
      '%;" aria-label="' +
      SOW_t("hud.music") +
      '">' +
      "      </label>" +
      '      <label class="sow-hud__setting-row sow-hud__setting-row--slider sow-hud__setting-card sow-hud__setting-card--slider">' +
      '        <div class="sow-menu__slider-label"><span class="sow-menu__settings-row-label">' +
      SOW_t("hud.sfx") +
      '</span><b id="sow-hud-sfx-value">' +
      initialSfxPct +
      "%</b></div>" +
      '        <input type="range" min="0" max="1" step="0.01" value="' +
      initialSfxVol +
      '" data-hud-setting="sfx_volume" style="--sow-slider-fill: ' +
      initialSfxPct +
      '%;" aria-label="' +
      SOW_t("hud.sfx") +
      '">' +
      "      </label>" +
      '      <label class="sow-hud__setting-row sow-menu__settings-switch-row"><span class="sow-menu__settings-row-label">' +
      SOW_t("hud.reduced_motion") +
      '</span><input class="sow-hud__switch" type="checkbox" role="switch" data-hud-setting="reduced_motion"></label>' +
      '      <label class="sow-hud__setting-row sow-menu__settings-switch-row"><span class="sow-menu__settings-row-label">' +
      SOW_t("hud.free_zoom_out") +
      '</span><input class="sow-hud__switch" type="checkbox" role="switch" data-hud-setting="free_zoom_out"></label>' +
      '      <label class="sow-hud__setting-row sow-menu__settings-switch-row"><span class="sow-menu__settings-row-label">' +
      SOW_t("hud.show_fps_ping") +
      '</span><input class="sow-hud__switch" type="checkbox" role="switch" data-hud-setting="show_fps_ping"></label>' +
      '      <label class="sow-hud__setting-row sow-menu__settings-switch-row"><span class="sow-menu__settings-row-label">' +
      esc(SOW_t("hud.show_match_clock")) +
      '</span><input class="sow-hud__switch" type="checkbox" role="switch" data-hud-setting="show_match_clock"></label>' +
      (hudState &&
      hudState.settings &&
      hudState.settings.show_dev_tools_available
        ? '      <label class="sow-hud__setting-row sow-menu__settings-switch-row"><span class="sow-menu__settings-row-label">' +
          SOW_t("hud.show_dev_tools") +
          '</span><input class="sow-hud__switch" type="checkbox" role="switch" data-hud-setting="show_dev_tools"></label>'
        : "") +
      renderLocaleSettingRow("hud-settings-language") +
      "    </section>" +
      "  </div>" +
      "</aside>" +
      '<div class="sow-hud__dock-stack" id="sow-hud-dock-stack">' +
      '<footer class="sow-hud__dock" id="sow-hud-dock">' +
      '  <div class="sow-hud__dock-inner" id="sow-hud-dock-inner">' +
      '    <div class="sow-hud__dock-actions">' +
      '      <div class="sow-hud__deploy-panel hidden" id="sow-hud-deploy-btn" data-spawned="false" data-urgency="normal">' +
      '        <span class="sow-hud__deploy-badge" aria-hidden="true">' +
      hudIcon("spawn", "sow-hud__deploy-icon") +
      "        </span>" +
      '        <div class="sow-hud__deploy-body">' +
      '          <div class="sow-hud__deploy-header">' +
      '            <span class="sow-hud__deploy-title">' +
      SOW_t("hud.choose_spawn") +
      "</span>" +
      '            <span class="sow-hud__deploy-status" id="sow-hud-deploy-status" hidden>' +
      SOW_t("hud.ready") +
      "</span>" +
      "          </div>" +
      '          <div class="sow-hud__deploy-track" aria-hidden="true">' +
      '            <i class="sow-hud__deploy-fill" id="sow-hud-deploy-fill"></i>' +
      "          </div>" +
      "        </div>" +
      '        <div class="sow-hud__deploy-timer-wrap">' +
      '          <b class="sow-hud__deploy-timer" id="sow-hud-deploy-timer" role="timer" aria-live="off">' +
      SOW_t("hud.ready") +
      "</b>" +
      "        </div>" +
      "      </div>" +
      '      <div class="sow-hud__buildings-strip" id="sow-hud-buildings-strip">' +
      ["City", "Farm", "Factory", "Port", "Tower", "SAM", "MissileSilo"].map((kind, index) => {
        const hotkey = index + 1;
        return '<button type="button" class="sow-hud__building-btn" data-command="select_building" data-kind="' +
          kind + '" aria-label="' + escapeHudText(SOW_t("hud.map_action_" + kind.toLowerCase())) +
          '" aria-keyshortcuts="' + hotkey + '">' +
          buildingIcon(kind) + tileCost(true) + dockHotkey(hotkey) + "</button>";
      }).join("") +
      "</div>" +
      '<div class="sow-hud__buildings-strip" id="sow-hud-nuclear-strip" style="display:none">' +
      [
        ["AtomBomb", "0"],
        ["HydrogenBomb", "9"],
        ["MIRV", "8"],
      ].map(([kind, hotkey], index) =>
        '<button type="button" class="sow-hud__building-btn" data-command="select_nuke" data-kind="' +
        kind + '" data-nuclear-index="' + index + '" aria-keyshortcuts="' + hotkey + '">' +
        '<span class="sow-hud__nuclear-label">' +
        escapeHudText(SOW_t("hud.nuclear_" + kind.toLowerCase())) + "</span>" +
        tileCost(true) + dockHotkey(hotkey) + "</button>"
      ).join("") +
      "      </div>" +
      '<div class="sow-hud__fleet-control" id="sow-hud-fleet-control" style="display:none" role="region" aria-label="Warship">' +
      '  <div class="sow-hud__fleet-control-head">' +
      unitIcon("warship", "sow-hud__fleet-control-icon") +
      '    <span class="sow-hud__fleet-control-count" data-fleet-count>×1</span>' +
      '    <span class="sow-hud__fleet-control-mode" data-fleet-mode></span>' +
      '    <button type="button" class="sow-hud__fleet-control-close" data-fleet-order="clear" aria-label="Close">×</button>' +
      "  </div>" +
      '  <div class="sow-hud__fleet-control-actions">' +
      '    <button type="button" class="sow-hud__fleet-order-btn" data-fleet-order="move">' +
      '      <span class="sow-hud__fleet-order-label">' + escapeHudText(SOW_t("hud.fleet_order_move")) + "</span>" +
      '      <kbd class="sow-hud__fleet-order-key">M</kbd>' +
      "    </button>" +
      '    <button type="button" class="sow-hud__fleet-order-btn" data-fleet-order="patrol">' +
      '      <span class="sow-hud__fleet-order-label">' + escapeHudText(SOW_t("hud.fleet_order_patrol")) + "</span>" +
      '      <kbd class="sow-hud__fleet-order-key">P</kbd>' +
      "    </button>" +
      "  </div>" +
      '  <div class="sow-hud__fleet-control-hint hidden" data-fleet-hint>' +
      escapeHudText(SOW_t("hud.fleet_order_move_hint")) +
      "  </div>" +
      "</div>" +
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
      '    <div class="sow-hud__sheet-title-group">' +
      '      <span class="sow-hud__sheet-title-badge" aria-hidden="true">' +
      hudIcon("rankings", "sow-hud__sheet-title-icon") +
      "      </span>" +
      '      <h3 class="sow-hud__sheet-title">' +
      SOW_t("hud.rankings_title") +
      "      </h3>" +
      "    </div>" +
      '    <div class="sow-hud__leaderboard-search-wrap sow-control-search">' +
      '      <span class="sow-hud__leaderboard-search-icon sow-control-search__icon" aria-hidden="true">' +
      hudIcon("search", "sow-hud__inline-icon") +
      "</span>" +
      '      <input class="sow-hud__leaderboard-search sow-control-search__input" id="sow-hud-lb-search" type="search" placeholder="' +
      SOW_t("hud.search_rankings") +
      '" aria-label="' +
      SOW_t("hud.search_rankings") +
      '" autocomplete="off" spellcheck="false" />' +
      "    </div>" +
      '    <button class="sow-hud__close-btn sow-hud__sheet-btn" type="button" data-command="toggle_leaderboard" aria-label="' +
      SOW_t("hud.close_rankings") +
      '">' +
      hudIcon("close", "sow-hud__close-icon") +
      "</button>" +
      "  </div>" +
      renderLeaderboardToolbar() +
      '  <div class="sow-hud__sheet-body sow-hud__leaderboard-rows" id="sow-hud-lb-rows"></div>' +
      '  <p class="sow-hud__leaderboard-empty" id="sow-hud-lb-empty" hidden>' +
      SOW_t("hud.no_rankings_found") +
      "</p>" +
      "</aside>" +
      '<aside class="sow-hud__inbox sow-hud__sheet hidden" id="sow-hud-inbox" role="dialog" aria-modal="false" aria-labelledby="sow-hud-inbox-title">' +
      '  <div class="sow-hud__sheet-header sow-hud__inbox-header">' +
      '    <div class="sow-hud__sheet-title-group">' +
      '      <span class="sow-hud__sheet-title-badge" aria-hidden="true">' +
      hudIcon("ring_request", "sow-hud__sheet-title-icon") +
      "      </span>" +
      '      <h3 class="sow-hud__sheet-title" id="sow-hud-inbox-title">' +
      SOW_t("hud.inbox") +
      "      </h3>" +
      "    </div>" +
      '<span class="sow-hud__inbox-total" id="sow-hud-inbox-total" aria-hidden="true">0</span>' +
      '<button class="sow-hud__close-btn sow-hud__sheet-btn" id="sow-hud-inbox-close" type="button" data-command="toggle_inbox" aria-label="' +
      SOW_t("hud.close_inbox") +
      '">' +
      hudIcon("close", "sow-hud__close-icon") +
      "</button></div>" +
      '  <div class="sow-hud__inbox-filters" id="sow-hud-inbox-filters" role="tablist" aria-label="' +
      SOW_t("hud.inbox") +
      '">' +
      '    <button class="sow-hud__inbox-filter is-active" type="button" role="tab" aria-selected="true" data-command="filter_inbox" data-inbox-filter="all">' +
      '      <span class="sow-hud__inbox-filter-label">' +
      SOW_t("hud.inbox_filter_all") +
      "</span>" +
      '      <span class="sow-hud__inbox-filter-count" data-filter-count>0</span>' +
      "    </button>" +
      '    <button class="sow-hud__inbox-filter" type="button" role="tab" aria-selected="false" data-command="filter_inbox" data-inbox-filter="alliance">' +
      '      <span class="sow-hud__inbox-filter-label">' +
      SOW_t("hud.inbox_filter_alliances") +
      "</span>" +
      '      <span class="sow-hud__inbox-filter-count" data-filter-count>0</span>' +
      "    </button>" +
      '    <button class="sow-hud__inbox-filter" type="button" role="tab" aria-selected="false" data-command="filter_inbox" data-inbox-filter="aid">' +
      '      <span class="sow-hud__inbox-filter-label">' +
      SOW_t("hud.inbox_filter_aid") +
      "</span>" +
      '      <span class="sow-hud__inbox-filter-count" data-filter-count>0</span>' +
      "    </button>" +
      "  </div>" +
      '  <div class="sow-hud__inbox-bulk" id="sow-hud-inbox-bulk" hidden>' +
      '    <button class="sow-hud__request-reject sow-hud__inbox-bulk-btn" id="sow-hud-inbox-reject-all" type="button" data-command="reject_all_inbox">' +
      SOW_t("hud.reject_all") +
      "</button>" +
      '    <button class="sow-hud__request-accept sow-hud__inbox-bulk-btn" id="sow-hud-inbox-accept-all" type="button" data-command="accept_all_inbox">' +
      SOW_t("hud.accept_all") +
      "</button>" +
      "  </div>" +
      '  <div class="sow-hud__sheet-body sow-hud__inbox-body" id="sow-hud-inbox-body">' +
      '    <div class="sow-hud__inbox-list" id="sow-hud-inbox-rows"></div>' +
      '    <p class="sow-hud__inbox-empty" id="sow-hud-inbox-empty" hidden>' +
      SOW_t("hud.no_pending_requests") +
      "</p>" +
      "  </div>" +
      "</aside>" +
      '<aside class="sow-hud__dialog sow-hud__transfer hidden" id="sow-hud-transfer" role="dialog" aria-modal="false" aria-labelledby="sow-hud-transfer-title">' +
      '  <div class="sow-hud__sheet-header sow-hud__transfer-header">' +
      '    <div class="sow-hud__sheet-title-group">' +
      '      <span class="sow-hud__sheet-title-badge" aria-hidden="true">' +
      hudIcon("transfer", "sow-hud__sheet-title-icon") +
      "      </span>" +
      '      <h3 class="sow-hud__sheet-title" id="sow-hud-transfer-title">' +
      SOW_t("hud.resource_transfer") +
      "      </h3>" +
      "    </div>" +
      '<button class="sow-hud__close-btn sow-hud__sheet-btn" id="sow-hud-transfer-close" type="button" data-command="close_transfer" aria-label="' +
      SOW_t("hud.close_resource_transfer") +
      '">' +
      hudIcon("close", "sow-hud__close-icon") +
      "</button></div>" +
      '  <div class="sow-hud__dialog-body">' +
      '  <div class="sow-hud__dialog-participant" id="sow-hud-transfer-target"></div>' +
      '  <p class="sow-hud__transfer-stock">' +
      '<span class="sow-hud__transfer-stock-item">' +
      hudIcon("troops", "sow-hud__inline-icon") +
      '<b id="sow-hud-transfer-stock-troops">0</b><small>/ <span id="sow-hud-transfer-stock-max-troops">0</span></small></span>' +
      '<span class="sow-hud__transfer-stock-item"><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' +
      currencyAsset("gold") +
      '" alt=""><b id="sow-hud-transfer-stock-gold">0</b></span>' +
      "</p>" +
      '  <div class="sow-hud__transfer-fields">' +
      '    <div class="sow-hud__transfer-field">' +
      transferDirButton("troops") +
      "<label><span>" +
      hudIcon("troops", "sow-hud__inline-icon") +
      SOW_t("hud.troops") +
      '</span><input id="sow-hud-transfer-troops" type="number" inputmode="numeric" min="0" step="1" value="0"></label>' +
      transferPresetButtons("troops") +
      "</div>" +
      '    <div class="sow-hud__transfer-field">' +
      transferDirButton("gold") +
      '<label><span><img class="sow-hud__currency-icon sow-hud__currency-icon--gold" src="' +
      currencyAsset("gold") +
      '" alt="">' +
      SOW_t("hud.gold") +
      '</span><input id="sow-hud-transfer-gold" type="number" inputmode="numeric" min="0" step="1" value="0"></label>' +
      transferPresetButtons("gold") +
      "</div>" +
      "  </div>" +
      "  </div>" +
      '  <p class="sow-hud__transfer-hint" id="sow-hud-transfer-hint" aria-live="polite" hidden></p>' +
      '  <div class="sow-hud__transfer-actions">' +
      '<button class="sow-hud__transfer-cta sow-hud__transfer-send" id="sow-hud-transfer-send" type="button" data-command="send_resources">' +
      hudIcon("send", "sow-hud__cta-icon") +
      "<span>" +
      SOW_t("hud.send") +
      "</span></button>" +
      '<button class="sow-hud__transfer-cta sow-hud__transfer-request" id="sow-hud-transfer-request" type="button" data-command="request_resources">' +
      hudIcon("ring_request", "sow-hud__cta-icon") +
      "<span>" +
      SOW_t("hud.request") +
      "</span></button>" +
      '<button class="sow-hud__transfer-cta sow-hud__transfer-trade" id="sow-hud-transfer-trade" type="button" data-command="offer_trade">' +
      hudIcon("ring_send", "sow-hud__cta-icon") +
      "<span>" +
      SOW_t("hud.trade") +
      "</span></button>" +
      "</div>" +
      "</aside>" +
      '<div class="sow-hud__modal-backdrop sow-hud__betrayal-backdrop hidden" id="sow-hud-betrayal-modal">' +
      '  <section class="sow-hud__dialog sow-hud__betrayal" role="dialog" aria-modal="true" aria-labelledby="sow-hud-betrayal-title" aria-describedby="sow-hud-betrayal-copy">' +
      '    <div class="sow-hud__sheet-header sow-hud__transfer-header">' +
      '      <div class="sow-hud__sheet-title-group">' +
      '        <span class="sow-hud__sheet-title-badge sow-hud__betrayal-mark" aria-hidden="true">' +
      statusIcon("traitor", "sow-hud__sheet-title-icon") +
      "        </span>" +
      '        <h3 class="sow-hud__sheet-title" id="sow-hud-betrayal-title">' +
      SOW_t("hud.break_alliance") +
      "        </h3>" +
      "      </div>" +
      '<button class="sow-hud__close-btn sow-hud__sheet-btn" id="sow-hud-betrayal-close" type="button" data-command="cancel_betrayal" aria-label="' +
      SOW_t("endgame.cancel") +
      '">' +
      hudIcon("close", "sow-hud__close-icon") +
      "</button></div>" +
      '    <div class="sow-hud__dialog-body"><div class="sow-hud__dialog-participant" id="sow-hud-betrayal-target"></div><p class="sow-hud__betrayal-copy" id="sow-hud-betrayal-copy">' +
      SOW_t("hud.break_alliance_body") +
      "</p></div>" +
      '    <div class="sow-hud__transfer-actions sow-hud__betrayal-actions"><button class="sow-hud__transfer-request" id="sow-hud-betrayal-cancel" type="button" data-command="cancel_betrayal">' +
      SOW_t("hud.keep_alliance") +
      '</button><button class="sow-hud__betrayal-confirm" id="sow-hud-betrayal-confirm" type="button" data-command="confirm_betrayal">' +
      statusIcon("traitor", "sow-hud__inline-icon") +
      '<span id="sow-hud-betrayal-confirm-label">' +
      SOW_t("hud.attack") +
      "</span>" +
      "</button></div>" +
      "  </section>" +
      "</div>" +
      '<div class="sow-hud__endgame-backdrop hidden" id="sow-hud-surrender-modal">' +
      '  <section class="sow-hud__endgame-card sow-hud__exit-card" data-result="exit" role="dialog" aria-modal="true" aria-labelledby="sow-hud-surrender-banner">' +
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
      SOW_t("endgame.leave") +
      "</span></h2>" +
      "      </div>" +
      "    </div>" +
      "    " +
      endgameStatsMarkup("surrender") +
      '    <div class="sow-hud__endgame-actions">' +
      '      <button class="sow-hud__endgame-secondary" type="button" data-command="close_surrender_modal" id="sow-hud-surrender-cancel">' +
      SOW_t("endgame.cancel") +
      "</button>" +
      '      <button class="sow-hud__endgame-primary" type="button" data-command="confirm_surrender">' +
      hudIcon("surrender", "sow-hud__inline-icon") +
      ' <span id="sow-hud-surrender-action-label">' +
      SOW_t("endgame.leave") +
      "</span></button>" +
      "    </div>" +
      "  </section>" +
      "</div>" +
      '<div class="sow-hud__endgame-backdrop hidden" id="sow-hud-endgame-modal">' +
      '  <section class="sow-hud__endgame-card" role="dialog" aria-modal="true" aria-labelledby="sow-hud-endgame-banner" aria-describedby="sow-hud-endgame-reason">' +
      '    <div class="sow-hud__endgame-fx" aria-hidden="true">' +
      '      <span class="sow-hud__endgame-rays"></span>' +
      '      <span class="sow-hud__endgame-shockwave"></span>' +
      "    </div>" +
      '    <div class="sow-hud__endgame-crest" aria-hidden="true">' +
      '      <span class="sow-hud__endgame-ribbon is-left"></span>' +
      '      <svg class="sow-hud__endgame-emblem is-victory" viewBox="0 0 64 40" fill="none">' +
      '        <path d="M8 24C14 22 19 18 22 12C19 20 15 26 8 29Z" fill="currentColor" opacity="0.85"/>' +
      '        <path d="M56 24C50 22 45 18 42 12C45 20 49 26 56 29Z" fill="currentColor" opacity="0.85"/>' +
      '        <path d="M32 4L39 14L48 11L43 24H21L16 11L25 14Z" stroke="currentColor" stroke-width="2" fill="currentColor" fill-opacity="0.22"/>' +
      '        <polygon points="32,12 36,19 32,26 28,19" fill="currentColor"/>' +
      "      </svg>" +
      '      <svg class="sow-hud__endgame-emblem is-defeat" viewBox="0 0 64 40" fill="none">' +
      '        <path d="M10 15C16 18 21 22 23 28C18 24 14 20 10 18Z" fill="currentColor" opacity="0.75"/>' +
      '        <path d="M54 15C48 18 43 22 41 28C46 24 50 20 54 18Z" fill="currentColor" opacity="0.75"/>' +
      '        <path d="M22 8H42L40 23L32 33L24 23Z" stroke="currentColor" stroke-width="2" fill="currentColor" fill-opacity="0.18"/>' +
      '        <path d="M34 9L29 18L35 22L30 31" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>' +
      "      </svg>" +
      '      <span class="sow-hud__endgame-ribbon is-right"></span>' +
      "    </div>" +
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
      '<p class="sow-hud__endgame-reason" id="sow-hud-endgame-reason" hidden></p>' +
      "      </div>" +
      "    </div>" +
      "    " +
      endgameStatsMarkup("endgame") +
      '    <div class="sow-hud__endgame-actions">' +
      '      <button class="sow-hud__endgame-secondary hidden" type="button" data-command="continue_observing" id="sow-hud-endgame-observe"><span aria-hidden="true">◉</span> ' +
      SOW_t("hud.continue_observing") +
      "</button>" +
      '      <button class="sow-hud__endgame-primary" type="button" data-command="confirm_endgame_leave">' +
      hudIcon("home", "sow-hud__inline-icon") +
      " " +
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
      hudDevLayer.appendChild(
        requiredHudQuery(hudRoot, "#sow-hud-dev-sidebar"),
      );
    }

    hudRefs = createHudRefs();

    hudRefs.transferTarget!.appendChild(hudRefs.transferIdentity.element);
    hudRefs.betrayalTarget!.appendChild(hudRefs.betrayalIdentity.element);

    hudRefs.notifications.className = "sow-hud__notifications";
    hudRefs.notifications.setAttribute("aria-live", "polite");
    hudRoot!.appendChild(hudRefs.notifications);
    hudRefs.requestToasts.id = "sow-hud-request-toasts";
    hudRefs.requestToasts.className = "sow-hud__request-toasts";
    hudRefs.requestToasts.setAttribute("role", "region");
    hudRefs.requestToasts.setAttribute("aria-live", "polite");
    hudRefs.requestToasts.setAttribute("aria-label", SOW_t("hud.inbox"));
    hudRefs.requestToasts.hidden = true;
    hudRefs.requestToastStack.className = "sow-hud__request-toast-stack";
    hudRefs.requestToastMore.type = "button";
    hudRefs.requestToastMore.className = "sow-hud__request-toast-more";
    hudRefs.requestToastMore.dataset.command = "open_inbox_from_toast";
    hudRefs.requestToastMore.hidden = true;
    hudRefs.requestToasts.append(
      hudRefs.requestToastStack,
      hudRefs.requestToastMore,
    );
    hudRoot!.appendChild(hudRefs.requestToasts);
    hudRefs.mapFeedback.className = "sow-hud__map-feedback";
    hudRefs.mapFeedback.setAttribute("aria-live", "polite");
    hudRefs.mapFeedback.hidden = true;
    hudRoot!.appendChild(hudRefs.mapFeedback);
    notificationCards = [];
    notificationCardsByKey.clear();
    leavingNotificationCards.forEach(function (entry) {
      if (entry.timer !== null) window.clearTimeout(entry.timer);
    });
    leavingNotificationCards.clear();
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
        const bucket = Math.round(val / 5);
        if (input.dataset.lastTickBucket !== String(bucket)) {
          input.dataset.lastTickBucket = String(bucket);
          playUiCue(AudioCue.UiSliderTick, 0, ratio);
        }
        const currentHud = hudState?.hud;
        updateArmyAllocation(currentHud?.troops, ratio);
        send("set_attack_ratio", { ratio: ratio });
      });
      if (allocationRail) {
        allocationRail.addEventListener("pointerenter", function (event) {
          if (event.pointerType !== "touch") refreshAllocationDetail();
        });
        allocationRail.addEventListener("focusin", function () {
          if (!allocationHoverNone || !allocationHoverNone.matches)
            refreshAllocationDetail();
        });
      }
    }
    hudDevLayer!.addEventListener("input", function (event) {
      const target = event.target;
      if (!(target instanceof Element)) return;
      const previewInput =
        target.closest<HTMLInputElement>("[data-dev-preview]");
      if (previewInput) {
        if (!(previewInput instanceof HTMLInputElement)) return;
        const field = previewInput.dataset.devPreview;
        if (!field || field === "target") return;
        const value =
          previewInput.type === "checkbox"
            ? previewInput.checked
            : Number(previewInput.value);
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
        renderDevControls(
          (hudState && hudState.hud && hudState.hud.dev_tools) || {},
        );
        return;
      }
      if (
        target.matches("[data-dev-picker]") &&
        target instanceof HTMLInputElement
      ) {
        sendDevControl(target);
        return;
      }
      const input = target.closest("[data-dev]");
      if (!(input instanceof HTMLInputElement)) return;
      sendDevControl(input);
    });
    hudDevLayer!.addEventListener("change", function (event) {
      const target = event.target;
      if (!(target instanceof Element)) return;
      if (target.matches("[data-dev-category-select]")) {
        const select = target as HTMLSelectElement;
        devTab = select.value;
        try {
          window.localStorage.setItem("sow_dev_tab", devTab);
        } catch {
          /* storage unavailable */
        }
        const devSearch =
          hudRefs!.devSidebar!.querySelector<HTMLInputElement>(
            "[data-dev-search]",
          );
        if (devSearch) devSearch.value = "";
        renderDevControls(
          (hudState && hudState.hud && hudState.hud.dev_tools) || {},
        );
        const devBody =
          hudRefs!.devSidebar.querySelector<HTMLElement>(".sow-hud__dev-body");
        if (devBody) devBody.scrollTop = 0;
      }
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

    const handleDevWheel = function (event: WheelEvent) {
      const target = event.target;
      if (!(target instanceof Element)) return;
      const sidebar = target.closest<HTMLElement>("#sow-hud-dev-sidebar");
      if (!sidebar) return;
      event.stopPropagation();
      const body = sidebar.querySelector<HTMLElement>(".sow-hud__dev-body");
      if (!body) return;
      const deltaY =
        event.deltaMode === 1
          ? event.deltaY * 24
          : event.deltaMode === 2
            ? event.deltaY * body.clientHeight
            : event.deltaY;
      body.scrollTop += deltaY;
      event.preventDefault();
    };

    if (hudDevLayer) {
      hudDevLayer.addEventListener("wheel", handleDevWheel, { passive: false });
    }

    ["pointerup", "pointercancel"].forEach(function (type) {
      window.addEventListener(
        type,
        function () {
          window.clearTimeout(devStepTimer);
        },
        { passive: true },
      );
    });

    hudRoot.addEventListener(
      "wheel",
      function (event: WheelEvent) {
        const target = event.target;
        if (!(target instanceof Element)) return;
        if (target.closest("#sow-hud-dev-sidebar")) {
          handleDevWheel(event);
          return;
        }
        const sheet = target.closest<HTMLElement>(".sow-hud__sheet");
        if (sheet) {
          event.stopPropagation();
          const body = sheet.querySelector<HTMLElement>(".sow-hud__sheet-body");
          if (body) {
            const deltaY =
              event.deltaMode === 1
                ? event.deltaY * 24
                : event.deltaMode === 2
                  ? event.deltaY * body.clientHeight
                  : event.deltaY;
            body.scrollTop += deltaY;
            event.preventDefault();
          }
        }
      },
      { passive: false },
    );
    const syncHudSliderVisual = (
      input: HTMLInputElement,
      valueSelector: string,
    ) => {
      const pct = Math.round(
        Math.max(0, Math.min(1, Number(input.value))) * 100,
      );
      input.style.setProperty("--sow-slider-fill", pct + "%");
      input.style.setProperty("--sow-music-fill", pct + "%");
      const valEl =
        hudRefs?.settings?.querySelector<HTMLElement>(valueSelector);
      if (valEl) {
        valEl.textContent = pct + "%";
      }
    };
    hudRoot.addEventListener("input", function (event) {
      const target = event.target;
      if (!(target instanceof Element)) return;
      const searchInput =
        target.closest<HTMLInputElement>("#sow-hud-lb-search");
      if (searchInput) {
        leaderboardFilterQuery = searchInput.value.trim().toLowerCase();
        applyLeaderboardFilter();
        return;
      }
      const input = target.closest<HTMLInputElement>(
        '[data-hud-setting="music_volume"], [data-hud-setting="sfx_volume"]',
      );
      if (input) {
        syncHudSliderVisual(
          input,
          input.dataset.hudSetting === "sfx_volume"
            ? "#sow-hud-sfx-value"
            : "#sow-hud-music-value",
        );
      }
    });
    hudRoot.addEventListener("search", function (event) {
      const target = event.target;
      if (!(target instanceof Element)) return;
      const searchInput =
        target.closest<HTMLInputElement>("#sow-hud-lb-search");
      if (searchInput) {
        leaderboardFilterQuery = searchInput.value.trim().toLowerCase();
        applyLeaderboardFilter();
      }
    });
    hudRoot.addEventListener("change", function (event) {
      const target = event.target;
      if (!(target instanceof Element)) return;
      const lbTeamInput = target.closest<HTMLInputElement>(
        "input[name='lb_team']",
      );
      if (lbTeamInput) {
        leaderboardTeamFilter =
          (lbTeamInput.value as "all" | "Red" | "Blue") || "all";
        const dropdown = lbTeamInput.closest<HTMLElement>(
          ".sow-control-dropdown",
        );
        if (dropdown)
          dropdown.classList.toggle(
            "is-filtered",
            leaderboardTeamFilter !== "all",
          );
        applyLeaderboardFilter();
        return;
      }
      const lbTypeInput = target.closest<HTMLInputElement>(
        "input[name='lb_type']",
      );
      if (lbTypeInput) {
        leaderboardTypeFilter =
          (lbTypeInput.value as "all" | "humans" | "bots") || "all";
        const dropdown = lbTypeInput.closest<HTMLElement>(
          ".sow-control-dropdown",
        );
        if (dropdown)
          dropdown.classList.toggle(
            "is-filtered",
            leaderboardTypeFilter !== "all",
          );
        applyLeaderboardFilter();
        return;
      }
      const lbSortInput = target.closest<HTMLInputElement>(
        "input[name='lb_sort']",
      );
      if (lbSortInput) {
        leaderboardSortMode =
          (lbSortInput.value as LeaderboardSortMode) || "rank";
        const dropdown = lbSortInput.closest<HTMLElement>(
          ".sow-control-dropdown",
        );
        if (dropdown)
          dropdown.classList.toggle(
            "is-filtered",
            leaderboardSortMode !== "rank",
          );
        leaderboardRenderKey = "";
        if (lastLeaderboardPlayersPayload) {
          updateLeaderboard(
            lastLeaderboardPlayersPayload,
            lastLeaderboardTeamsPayload || lastLeaderboardTeams,
          );
        }
        return;
      }
      const localeInput = target.closest<HTMLInputElement>(
        "[data-setting='locale']",
      );
      if (localeInput) {
        if (typeof window.SOW_setLocale === "function")
          void window.SOW_setLocale(localeInput.value);
        return;
      }
      const input = target.closest("[data-hud-setting]");
      if (!(input instanceof HTMLInputElement)) return;
      const setting = input.dataset.hudSetting;
      if (setting === "mute_all") {
        syncAudioSettings({ mute_all: !input.checked });
        send("set_mute", { value: !input.checked });
      }
      if (setting === "music_volume") {
        const val = Number(input.value);
        syncAudioSettings({ music_volume: val });
        syncHudSliderVisual(input, "#sow-hud-music-value");
        send("set_music_volume", { value: val });
      }
      if (setting === "sfx_volume") {
        const val = Number(input.value);
        syncAudioSettings({ sfx_volume: val });
        syncHudSliderVisual(input, "#sow-hud-sfx-value");
        const bucket = Math.round(val * 20);
        if (input.dataset.lastTickBucket !== String(bucket)) {
          input.dataset.lastTickBucket = String(bucket);
          playUiCue(AudioCue.UiSliderTick, 0, val);
        }
        send("set_sfx_volume", { value: val });
      }
      if (setting === "reduced_motion")
        send("set_reduced_motion", { value: input.checked });
      if (setting === "free_zoom_out")
        send("set_free_zoom_out", { value: input.checked });
      if (setting === "show_match_clock")
        send("set_show_match_clock", { value: input.checked });
      if (setting === "show_fps_ping")
        send("set_show_fps_ping", { value: input.checked });
      if (setting === "show_dev_tools") {
        if (!input.checked) {
          devSidebarOpen = false;
        }
        send("set_show_dev_tools", { value: input.checked });
      }
    });
  }

  const mapActionLabels: Record<
    string,
    { icon?: string; key: string }
  > = {
    emote: { icon: "ring_emote", key: "hud.map_action_emote" },
    spawn: { icon: "ring_spawn", key: "hud.map_action_deploy" },
    attack: { icon: "ring_attack", key: "hud.map_action_attack" },
    fleet: { icon: "ring_fleet", key: "hud.map_action_fleet" },
    send: { icon: "ring_send", key: "hud.map_action_send" },
    request: { icon: "ring_request", key: "hud.map_action_request" },
    request_allies: { icon: "ring_request", key: "hud.map_action_request_allies" },
    alliance: { icon: "ring_alliance", key: "hud.map_action_alliance" },
    build_city: { key: "hud.map_action_city" },
    build_sam: { key: "hud.map_action_sam" },
    build_missile_silo: { key: "hud.map_action_missilesilo" },
    upgrade_structure: { icon: "ring_upgrade", key: "hud.map_action_upgrade" },
    nuke: { icon: "ring_nuke", key: "hud.map_action_nuke" },
  };

  const allianceActionLabels: Record<
    string,
    { state: string; key: string; icon: string; disabled: boolean }
  > = {
    request: {
      state: "request",
      key: "hud.map_action_request_alliance",
      icon: "ring_alliance",
      disabled: false,
    },
    accept: {
      state: "accept",
      key: "hud.map_action_accept_alliance",
      icon: "ring_alliance",
      disabled: false,
    },
    pending: {
      state: "pending",
      key: "hud.map_action_alliance_pending",
      icon: "ring_pending",
      disabled: true,
    },
    active: {
      state: "active",
      key: "hud.map_action_alliance_active",
      icon: "ring_alliance",
      disabled: true,
    },
    renew: {
      state: "renew",
      key: "hud.map_action_renew_alliance",
      icon: "ring_alliance",
      disabled: false,
    },
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
    if (
      hudRefs.slider &&
      hudRefs.slider.style.getPropertyValue("--sow-ratio") !== fill
    )
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
    const allocationKey =
      total + ":" + share + ":" + String(window.SOW_LOCALE || "en");
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
    if (
      hudRefs.slider &&
      hudRefs.slider.getAttribute("aria-valuetext") !== text
    ) {
      hudRefs.slider.setAttribute("aria-valuetext", text);
    }
  }

  function mapActionReason(
    item: HudActionReasonItem,
    availableGold: number | undefined,
  ): string {
    if (item.reason_key) {
      let reason = SOW_t(
        item.reason_key,
        item.reason_troops == null
          ? {}
          : { troops: Math.ceil(Number(item.reason_troops)).toLocaleString() },
      );
      if (item.reason_level != null || item.reason_seconds != null) {
        if (item.reason_key !== "lobbies.locked")
          reason = SOW_t("lobbies.locked") + " · " + reason;
        if (item.reason_level != null)
          reason += " " + Math.floor(Number(item.reason_level));
        if (item.reason_seconds != null)
          reason +=
            " · " + Math.max(1, Math.ceil(Number(item.reason_seconds))) + "s";
      }
      return reason;
    }
    if (!item.disabled || item.cost == null) return "";
    return SOW_t("hud.not_enough_gold", {
      cost: Math.ceil(item.cost).toLocaleString(),
      available: Math.floor(Number(availableGold) || 0).toLocaleString(),
    });
  }

  function formatAllianceCountdown(value: number): string {
    const seconds = Math.max(0, Math.ceil(value));
    const minutesText = String(Math.floor(seconds / 60)).padStart(2, "0");
    const secondsText = String(seconds % 60).padStart(2, "0");
    return minutesText + ":" + secondsText;
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
    return (
      unlocks === null ||
      window.SOWCampaign.campaignAllowsBuilding(unlocks, kind, level)
    );
  }
  function campaignAllowsAction(
    unlocks: { buildings: Record<string, number>; actions: string[] } | null,
    action: string,
  ): boolean {
    return (
      unlocks === null ||
      window.SOWCampaign.campaignAllowsAction(unlocks, action)
    );
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
        level:
          item && item.level != null && Number.isFinite(Number(item.level))
            ? Number(item.level)
            : null,
        disabled: Boolean(item && item.disabled),
        reason_key: (item && item.reason_key) || null,
        reason_level:
          item &&
          item.reason_level != null &&
          Number.isFinite(Number(item.reason_level))
            ? Number(item.reason_level)
            : null,
        reason_seconds:
          item &&
          item.reason_seconds != null &&
          Number.isFinite(Number(item.reason_seconds))
            ? Number(item.reason_seconds)
            : null,
        reason_troops:
          item &&
          item.reason_troops != null &&
          Number.isFinite(Number(item.reason_troops))
            ? Number(item.reason_troops)
            : null,
      };
    });
  }

  function patchBuildingProductionButtons(
    menu: HTMLElement,
    production: HudBuildingProduction,
    busy: boolean,
    availableGold: number | undefined,
  ): void {
    const warshipBtn = menu.querySelector<HTMLButtonElement>(
      '[data-production-kind="Warship"]',
    );
    if (warshipBtn && production.warship) {
      const warship = production.warship;
      const costText = formatHudMetric(Math.max(0, Math.ceil(Number(warship.cost) || 0)));
      const timeText = Math.max(1, Math.ceil(Number(warship.seconds) || 0)) + "s";
      const cost = warshipBtn.querySelector<HTMLElement>("[data-production-cost]");
      const time = warshipBtn.querySelector<HTMLElement>("[data-production-time]");
      if (cost && cost.textContent !== costText) cost.textContent = costText;
      if (time && time.textContent !== timeText) time.textContent = timeText;
      const queueLen = Math.max(0, Math.floor(Number(warship.queue_len) || 0));
      const queueFull = Boolean(warship.queue_full);
      const badgeEl = warshipBtn.querySelector<HTMLElement>("[data-warship-badge]");
      if (badgeEl) {
        const badgeText = queueLen > 0 ? "×" + queueLen : "";
        if (badgeEl.textContent !== badgeText) badgeEl.textContent = badgeText;
        badgeEl.classList.toggle("hidden", queueLen === 0);
      }
      const enabled = Boolean(warship.enabled);
      warshipBtn.disabled = false;
      warshipBtn.classList.toggle("is-locked", !enabled);
      warshipBtn.classList.toggle("is-queued", queueLen > 0);
      warshipBtn.setAttribute("aria-disabled", String(!enabled));
      const reason = !warship.campaign_unlocked
        ? SOW_t("hud.campaign_unlock_required")
        : queueFull
          ? SOW_t("hud.warship_queue_full")
          : !warship.capacity_ok
            ? SOW_t("hud.fleet_capacity_full")
            : !warship.affordable
              ? mapActionReason({ disabled: true, cost: warship.cost }, availableGold) || ""
              : "";
      const title = SOW_t("hud.map_action_build_warship");
      const tip = reason || costText + "g · " + timeText;
      const label = title + " · " + tip;
      if (warshipBtn.getAttribute("aria-label") !== label) warshipBtn.setAttribute("aria-label", label);
      if (warshipBtn.dataset.sowTooltipKey !== tip) {
        warshipBtn.dataset.sowTooltipKey = tip;
        SOWTooltip.set(warshipBtn, { title, text: tip, tone: reason ? "danger" : "gold" });
      }

      const maxBtn = menu.querySelector<HTMLButtonElement>('[data-production-max="Warship"]');
      if (maxBtn) {
        const maxCount = Math.max(0, Math.floor(Number(warship.max_count) || 0));
        const maxCost = Math.max(0, Math.ceil(Number(warship.max_cost) || 0));
        const maxEnabled = maxCount > 0;
        const maxLabelEl = maxBtn.querySelector<HTMLElement>("[data-production-max-count]");
        const maxCountText = maxEnabled ? "×" + maxCount : "";
        if (maxLabelEl && maxLabelEl.textContent !== maxCountText) maxLabelEl.textContent = maxCountText;
        const maxCostEl = maxBtn.querySelector<HTMLElement>("[data-production-max-cost]");
        const maxCostText = maxEnabled ? formatHudMetric(maxCost) : "—";
        if (maxCostEl && maxCostEl.textContent !== maxCostText) maxCostEl.textContent = maxCostText;
        maxBtn.classList.toggle("is-locked", !maxEnabled);
        maxBtn.setAttribute("aria-disabled", String(!maxEnabled));
      }

      const queueItems = Array.isArray(warship.queue) ? warship.queue : [];
      menu.querySelectorAll<HTMLButtonElement>("[data-cancel-warship-index]").forEach(function (slotBtn) {
        const idx = Number(slotBtn.dataset.cancelWarshipIndex);
        const item = queueItems[idx];
        const filled = Boolean(item);
        slotBtn.classList.toggle("is-filled", filled);
        slotBtn.classList.toggle("is-active", idx === 0 && filled);
        slotBtn.disabled = !filled;
        const bar = slotBtn.querySelector<HTMLElement>("[data-queue-progress]");
        if (bar) {
          const pct = idx === 0 && item ? Math.max(0, Math.min(100, (Number(item.progress) || 0) * 100)) : 0;
          bar.style.width = pct.toFixed(1) + "%";
        }
        const remEl = slotBtn.querySelector<HTMLElement>("[data-queue-time]");
        if (remEl) {
          const remText = idx === 0 && item ? Math.max(1, Math.ceil(Number(item.remaining_seconds) || 0)) + "s" : "";
          if (remEl.textContent !== remText) remEl.textContent = remText;
        }
        if (item) {
          const refundText = SOW_t("hud.warship_cancel_refund", {
            gold: formatHudMetric(Math.max(0, Math.round(Number(item.paid_gold) || 0))),
          });
          if (slotBtn.dataset.sowTooltipKey !== refundText) {
            slotBtn.dataset.sowTooltipKey = refundText;
            SOWTooltip.set(slotBtn, { title: SOW_t("hud.fleet_warship_label"), text: refundText });
          }
        } else if (slotBtn.dataset.sowTooltipKey) {
          delete slotBtn.dataset.sowTooltipKey;
          SOWTooltip.clear(slotBtn);
        }
      });
    }

    const nukeBtn = menu.querySelector<HTMLButtonElement>(
      '[data-production-kind="Nuke"], [data-command="select_nuke"]',
    );
    if (nukeBtn && production.nuke) {
      const nuke = production.nuke;
      const title = SOW_t("hud.map_action_nuke");
      if (nuke.can_launch) {
        nukeBtn.disabled = false;
        nukeBtn.classList.remove("is-locked");
        nukeBtn.classList.add("is-ready");
        nukeBtn.classList.toggle("active", Boolean(nuke.selected));
        nukeBtn.setAttribute("aria-disabled", "false");
        nukeBtn.setAttribute("aria-pressed", String(Boolean(nuke.selected)));
        const readyText = SOW_t("hud.ready");
        const status = nukeBtn.querySelector<HTMLElement>("[data-production-status]");
        if (status && status.textContent !== readyText) status.textContent = readyText;
        const label = title + " · " + readyText;
        if (nukeBtn.getAttribute("aria-label") !== label) nukeBtn.setAttribute("aria-label", label);
        if (nukeBtn.dataset.sowTooltipKey !== readyText) {
          nukeBtn.dataset.sowTooltipKey = readyText;
          SOWTooltip.set(nukeBtn, { title, text: readyText, tone: "gold" });
        }
      } else {
        const costText = formatHudMetric(Math.max(0, Math.ceil(Number(nuke.cost) || 0)));
        const timeText = Math.max(1, Math.ceil(Number(nuke.seconds) || 0)) + "s";
        const cost = nukeBtn.querySelector<HTMLElement>("[data-production-cost]");
        const time = nukeBtn.querySelector<HTMLElement>("[data-production-time]");
        if (cost && cost.textContent !== costText) cost.textContent = costText;
        if (time && time.textContent !== timeText) time.textContent = timeText;
        const enabled = Boolean(nuke.can_build) && !busy;
        nukeBtn.disabled = false;
        nukeBtn.classList.remove("is-ready", "active");
        nukeBtn.classList.toggle("is-locked", !enabled);
        nukeBtn.setAttribute("aria-disabled", String(!enabled));
        const reason = !nuke.campaign_unlocked
          ? SOW_t("hud.campaign_unlock_required")
          : busy
            ? SOW_t("hud.building_in_progress")
            : !nuke.affordable
              ? mapActionReason({ disabled: true, cost: nuke.cost }, availableGold) || ""
              : "";
        const tip = reason || costText + "g · " + timeText;
        const label = title + " · " + tip;
        if (nukeBtn.getAttribute("aria-label") !== label) nukeBtn.setAttribute("aria-label", label);
        if (nukeBtn.dataset.sowTooltipKey !== tip) {
          nukeBtn.dataset.sowTooltipKey = tip;
          SOWTooltip.set(nukeBtn, { title, text: tip, tone: reason ? "danger" : "gold" });
        }
      }
    }
  }

  function patchBuildingDetailsCard(
    menu: HTMLElement,
    building: HudMapMenuBuilding,
    availableGold: number | undefined,
  ): void {
    const construction = building.construction;
    const warshipLoader = menu.querySelector<HTMLElement>("[data-port-warship-progress]");
    if (warshipLoader) {
      const activeWarship = building.production?.warship?.queue?.[0];
      warshipLoader.classList.toggle("is-idle", !activeWarship);
      if (activeWarship)
        patchBuildingProgressBar(warshipLoader, activeWarship.remaining_seconds, activeWarship.progress);
    } else if (
      construction ||
      building.under_construction ||
      building.remaining_seconds != null
    ) {
      patchBuildingProgressBar(
        menu,
        construction ? construction.remaining_seconds : building.remaining_seconds,
        construction ? construction.progress : building.progress,
      );
    }
    const busy = Boolean(construction || building.under_construction);
    if (building.production) {
      patchBuildingProductionButtons(menu, building.production, busy, availableGold);
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
    const button = menu.querySelector<HTMLButtonElement>(
      ".sow-hud__building-upgrade-btn:not(.is-max-offer)",
    );
    if (button) {
      button.disabled = false;
      button.classList.toggle("is-locked", !enabled);
      button.setAttribute("aria-disabled", String(!enabled));
      const baseLabel =
        SOW_t("hud.map_action_upgrade") + " Lv." + upgrade.next_level;
      const reason = !enabled
        ? mapActionReason({ disabled: true, cost: upgrade.cost }, availableGold)
        : costText + "g · " + timeText;
      const accessibleLabel = reason ? baseLabel + ". " + reason : baseLabel;
      if (button.getAttribute("aria-label") !== accessibleLabel) {
        button.setAttribute("aria-label", accessibleLabel);
      }
    }
  }

  function renderBuildingProgressBarMarkup(
    kind: string,
    level: number,
  ): string {
    return (
      '<div class="sow-hud__building-progress-row" role="status">' +
      '<span class="sow-hud__building-card-thumb" aria-hidden="true">' +
      structureOrUnitIcon(
        kind,
        Math.max(1, level),
        "sow-hud__building-card-art",
      ) +
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
    const timerEl = container.querySelector<HTMLElement>(
      "[data-building-timer]",
    );
    const secsText =
      Math.max(1, Math.ceil(Number(remainingSeconds) || 0)) + "s";
    if (timerEl && timerEl.textContent !== secsText) {
      timerEl.textContent = secsText;
    }
    const fillEl = container.querySelector<HTMLElement>(
      "[data-building-progress-fill]",
    );
    if (fillEl) {
      const ratio = Math.max(0, Math.min(1, Number(progress) || 0));
      const scaleText = "scaleX(" + ratio.toFixed(4) + ")";
      if (fillEl.style.transform !== scaleText) {
        fillEl.style.transform = scaleText;
      }
    }
  }

  function positionBuildingDetailsCard(
    menu: HTMLElement,
    mapMenu: HudMapMenu,
  ): void {
    const bld = mapMenu.building;
    const anchorX = Number.isFinite(Number(bld && bld.anchor_x))
      ? Number(bld!.anchor_x)
      : Number(mapMenu.x || 0);
    const anchorY = Number.isFinite(Number(bld && bld.anchor_y))
      ? Number(bld!.anchor_y)
      : Number(mapMenu.y || 0);
    const anchorRadius = Math.max(20, Number(bld && bld.anchor_radius) || 28);
    const card =
      menu.querySelector<HTMLElement>(".sow-hud__building-card") || menu;
    const viewport = window.visualViewport;
    const viewLeft = (viewport && viewport.offsetLeft) || 0;
    const viewTop = (viewport && viewport.offsetTop) || 0;
    const vw = (viewport && viewport.width) || window.innerWidth || 800;
    const vh = (viewport && viewport.height) || window.innerHeight || 600;
    const viewportWidth = vw + "px";
    if (
      menu.style.getPropertyValue("--sow-hud-building-card-viewport-width") !==
      viewportWidth
    ) {
      menu.style.setProperty(
        "--sow-hud-building-card-viewport-width",
        viewportWidth,
      );
    }
    const width = card.offsetWidth || 232;
    const halfW = width * 0.5;
    let bottomLimit = viewTop + vh - 96;
    if (
      hudRefs &&
      hudRefs.dock &&
      !hudRefs.dock.hidden &&
      typeof hudRefs.dock.getBoundingClientRect === "function"
    ) {
      const dockRect = hudRefs.dock.getBoundingClientRect();
      if (dockRect && dockRect.height > 0 && dockRect.top > viewTop) {
        bottomLimit = Math.min(bottomLimit, dockRect.top - 10);
      }
    }
    const availableHeight = Math.max(0, Math.min(vh, bottomLimit - viewTop));
    const viewportHeight = availableHeight + "px";
    if (
      menu.style.getPropertyValue("--sow-hud-building-card-viewport-height") !==
      viewportHeight
    ) {
      menu.style.setProperty(
        "--sow-hud-building-card-viewport-height",
        viewportHeight,
      );
    }
    const height = card.offsetHeight || Math.max(1, availableHeight - 20);
    const halfH = height * 0.5;
    const gap = 10;
    const belowCenterY = anchorY + anchorRadius + gap + halfH;
    const aboveCenterY = anchorY - anchorRadius - gap - halfH;
    const placeBelow =
      belowCenterY + halfH <= bottomLimit ||
      aboveCenterY - halfH < viewTop + 12;
    const rawCenterY = placeBelow ? belowCenterY : aboveCenterY;
    const minX = viewLeft + halfW + 10;
    const maxX = Math.max(minX, viewLeft + vw - halfW - 10);
    const minY = viewTop + halfH + 10;
    const maxY = Math.max(
      minY,
      Math.min(viewTop + vh - halfH - 10, bottomLimit - halfH),
    );
    const left = Math.min(Math.max(anchorX, minX), maxX);
    const top = Math.min(Math.max(rawCenterY, minY), maxY);
    menu.style.left = left.toFixed(1) + "px";
    menu.style.top = top.toFixed(1) + "px";
    menu.dataset.placement = placeBelow ? "below" : "above";
    const caretLimit = Math.max(0, halfW - 18);
    const caretOffsetX = Math.max(
      -caretLimit,
      Math.min(caretLimit, anchorX - left),
    );
    menu.style.setProperty("--caret-offset-x", caretOffsetX.toFixed(1) + "px");
  }

  function renderBuildingProductionGrid(
    buildingId: number,
    production: HudBuildingProduction,
  ): string {
    const buttons: string[] = [];
    if (production.warship) {
      const queueSlots = [0, 1, 2, 3, 4]
        .map(
          (idx) =>
            '<button type="button" class="sow-hud__warship-slot" data-cancel-warship-index="' +
            idx +
            '" data-building-id="' +
            buildingId +
            '" disabled>' +
            unitIcon("warship", "sow-hud__warship-slot-art") +
            '<span class="sow-hud__warship-slot-time" data-queue-time></span>' +
            '<span class="sow-hud__warship-slot-x">×</span>' +
            '<span class="sow-hud__warship-slot-bar"><i data-queue-progress></i></span>' +
            "</button>",
        )
        .join("");
      buttons.push(
        '<div class="sow-hud__warship-prod-row">' +
          '<button type="button" class="sow-hud__city-arsenal-btn sow-hud__warship-build-btn" data-production-kind="Warship" data-building-id="' +
          buildingId +
          '">' +
          unitIcon("warship", "sow-hud__city-arsenal-art") +
          '<span class="sow-hud__warship-queue-badge hidden" data-warship-badge></span>' +
          '<span class="sow-hud__city-arsenal-name">' +
          escapeHudText(SOW_t("hud.fleet_warship_label")) +
          "</span>" +
          '<span class="sow-hud__city-arsenal-meta">' +
          hudIcon("gold", "sow-hud__building-upgrade-meta-icon") +
          "<b data-production-cost></b>" +
          '<span class="sow-hud__city-arsenal-sep">·</span>' +
          "<b data-production-time></b>" +
          "</span>" +
          "</button>" +
          '<button type="button" class="sow-hud__city-arsenal-btn sow-hud__warship-max-btn" data-production-max="Warship" data-building-id="' +
          buildingId +
          '">' +
          '<span class="sow-hud__city-arsenal-name">' +
          escapeHudText(SOW_t("hud.warship_max_label")) +
          ' <em data-production-max-count></em></span>' +
          '<span class="sow-hud__city-arsenal-meta">' +
          hudIcon("gold", "sow-hud__building-upgrade-meta-icon") +
          "<b data-production-max-cost></b>" +
          "</span>" +
          "</button>" +
          "</div>" +
          '<div class="sow-hud__warship-queue" role="group" aria-label="Warship queue">' +
          queueSlots +
          "</div>",
      );
    }
    if (production.nuke) {
      const attrs = production.nuke.can_launch
        ? 'data-command="select_nuke"'
        : 'data-production-kind="Nuke" data-building-id="' + buildingId + '"';
      const meta = production.nuke.can_launch
        ? '<span class="sow-hud__city-arsenal-meta is-ready" data-production-status></span>'
        : '<span class="sow-hud__city-arsenal-meta">' +
          hudIcon("gold", "sow-hud__building-upgrade-meta-icon") +
          "<b data-production-cost></b>" +
          '<span class="sow-hud__city-arsenal-sep">·</span>' +
          "<b data-production-time></b>" +
          "</span>";
      buttons.push(
        '<button type="button" class="sow-hud__city-arsenal-btn" ' +
          attrs +
          ">" +
          unitIcon("atom-bomb", "sow-hud__city-arsenal-art") +
          '<span class="sow-hud__city-arsenal-name">' +
          escapeHudText(SOW_t("hud.map_action_nuke")) +
          "</span>" +
          meta +
          "</button>",
      );
    }
    return buttons.length
      ? '<div class="sow-hud__city-arsenal-grid">' + buttons.join("") + "</div>"
      : "";
  }

  function renderBuildingDetailsCard(
    menu: HTMLElement,
    mapMenu: HudMapMenu,
    hud: HudRingState,
  ): boolean {
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
    const visualLevel = Math.max(
      1,
      Math.floor(Number(building.visual_level || level) || 1),
    );
    const maxLevel = Math.max(
      level,
      Math.floor(Number(building.max_level) || level),
    );
    const construction = building.construction || null;
    const underConstruction =
      Boolean(building.under_construction) ||
      Boolean(construction) ||
      building.remaining_seconds != null;
    const isCityFoundation = Boolean(building.city_foundation);
    const production = building.production || null;
    const hasProductionPanel = Boolean(production);
    const chips = Array.isArray(building.chips) ? building.chips : [];
    const upgrade =
      !underConstruction && building.upgrade ? building.upgrade : null;
    const session = String(mapMenu.session) + ":" + String(mapMenu.tile_idx);
    menu.dataset.session = String(mapMenu.session);
    menu.dataset.tileIdx = String(mapMenu.tile_idx);
    delete menu.dataset.level;
    menu.classList.remove("hidden", "has-tether");
    menu.classList.add("is-building-card");

    const productionSig = production
      ? [
          production.warship ? "W" : "",
          production.nuke ? (production.nuke.can_launch ? "L" : "N") : "",
        ].join(":")
      : "";

    const renderKey = [
      window.SOW_LOCALE || "en",
      "building_details",
      session,
      kind,
      level,
      visualLevel,
      maxLevel,
      isCityFoundation ? "foundation" : hasProductionPanel ? "production" : "building",
      building.building_id ?? "",
      building.coastal ? 1 : 0,
      building.railway_connected ? 1 : 0,
      construction
        ? "c:" + construction.kind + ":" + construction.target_level
        : underConstruction
          ? "constructing"
          : upgrade
            ? "upgrade:" + upgrade.next_level + ":" + (upgrade.max ? upgrade.max.target_level : 0)
            : "max",
      chips
        .map(function (chip) {
          return chip.icon + "=" + chip.label;
        })
        .join("|"),
      productionSig,
    ].join(":");

    if (menu.dataset.renderKey !== renderKey) {
      const card = document.createElement("div");
      card.dataset.buildingKind = kind.toLowerCase();
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
          "</button>" +
          (upgrade.max
            ? '<button type="button" class="sow-hud__building-upgrade-btn is-max-offer" data-map-action="upgrade_structure_max">' +
              '<span class="sow-hud__building-upgrade-cta">' +
              statusIcon("arrow-up", "sow-hud__building-upgrade-arrow") +
              '<span class="sow-hud__building-upgrade-label">' +
              escapeHudText(
                upgrade.max.is_cap_max
                  ? SOW_t("hud.map_action_upgrade_max")
                  : "→",
              ) +
              " <em>Lv." +
              upgrade.max.target_level +
              "</em></span></span>" +
              '<span class="sow-hud__building-upgrade-meta">' +
              '<span class="sow-hud__building-upgrade-pill sow-hud__building-upgrade-cost">' +
              hudIcon("gold", "sow-hud__building-upgrade-meta-icon") +
              "<b>" +
              Math.ceil(upgrade.max.cost) +
              "</b></span>" +
              '<span class="sow-hud__building-upgrade-pill sow-hud__building-upgrade-time">' +
              statusIcon("clock", "sow-hud__building-upgrade-meta-icon") +
              "<b>" +
              Math.ceil(upgrade.max.seconds) +
              "s</b></span></span></button>"
            : "")
        : "";

      if (isCityFoundation || (!hasProductionPanel && underConstruction)) {
        // The compact world-anchored progress bar already shows the countdown on the building.
        menu.classList.add("hidden");
        menu.classList.remove("is-building-card", "has-tether");
        if (menu.dataset.renderKey) menu.replaceChildren();
        menu.dataset.renderKey = "";
        return false;
      } else if (hasProductionPanel) {
        const buildingTitle = SOW_t("hud.map_action_" + kind.toLowerCase());
        const traitsHtml =
          (building.coastal
            ? '<span class="sow-hud__city-trait is-coastal" title="Coastal">' +
              hudIcon("anchor", "sow-hud__city-trait-icon") +
              "</span>"
            : "") +
          (building.railway_connected
            ? '<span class="sow-hud__city-trait is-rail" title="Railway">' +
              hudIcon("speed", "sow-hud__city-trait-icon") +
              "</span>"
            : "");
        // Show in-card progress only for Arsenal builds (Warship/Nuke), not building upgrades
        // which already render the compact world-anchored bar.
        const isArsenalBuild = Boolean(
          construction && (construction.kind === "Warship" || construction.kind === "Nuke"),
        );
        const constructionHtml = production?.warship
          ? '<div class="sow-hud__city-construction is-idle" data-port-warship-progress>' +
            renderBuildingProgressBarMarkup("Warship", 1) +
            "</div>"
          : isArsenalBuild && construction
          ? '<div class="sow-hud__city-construction">' +
            renderBuildingProgressBarMarkup(
              construction.kind,
              construction.target_level,
            ) +
            "</div>"
          : "";
        const productionHtml = renderBuildingProductionGrid(
          Number(building.building_id) || 0,
          production || {},
        );
        card.className = "sow-hud__building-card is-city-card";
        card.innerHTML =
          '<span class="sow-hud__building-card-caret" aria-hidden="true"></span>' +
          '<div class="sow-hud__building-card-head">' +
          '<span class="sow-hud__building-card-thumb" aria-hidden="true">' +
          buildingIcon(
            kind,
            Math.max(1, visualLevel),
            "sow-hud__building-card-art",
          ) +
          "</span>" +
          '<div class="sow-hud__building-card-identity">' +
          '<strong class="sow-hud__building-card-title">' +
          escapeHudText(buildingTitle) +
          "</strong>" +
          '<div class="sow-hud__city-badges">' +
          traitsHtml +
          '<span class="sow-hud__building-card-level' +
          (visualLevel >= 4 ? " is-max" : "") +
          '">Lv.' +
          visualLevel +
          "</span>" +
          "</div>" +
          "</div>" +
          "</div>" +
          constructionHtml +
          chipsHtml +
          (production?.warship ? productionHtml + footerHtml : footerHtml + productionHtml);
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

  function positionPlacementOverlay(
    overlay: HTMLElement,
    placement: HudBuildingPlacement,
  ): void {
    const viewport = window.visualViewport;
    const viewLeft = (viewport && viewport.offsetLeft) || 0;
    const viewTop = (viewport && viewport.offsetTop) || 0;
    const vw = (viewport && viewport.width) || window.innerWidth || 800;
    const vh = (viewport && viewport.height) || window.innerHeight || 600;
    const anchorX = Number.isFinite(Number(placement.anchor_x))
      ? Number(placement.anchor_x)
      : viewLeft + vw * 0.5;
    const anchorY = Number.isFinite(Number(placement.anchor_y))
      ? Number(placement.anchor_y)
      : viewTop + vh * 0.5;
    const clamp = (value: number, min: number, max: number) =>
      Math.min(Math.max(value, min), Math.max(min, max));

    // Batch DOM reads before writing arrow styles to avoid forced synchronous reflow
    const panel = hudRefs && hudRefs.placementPanel;
    const width = panel ? panel.offsetWidth || 230 : 230;
    const height = panel ? panel.offsetHeight || 96 : 96;
    const halfW = width * 0.5;
    const halfH = height * 0.5;

    let bottomLimit = viewTop + vh - 96;
    if (
      hudRefs &&
      hudRefs.dock &&
      !hudRefs.dock.hidden &&
      typeof hudRefs.dock.getBoundingClientRect === "function"
    ) {
      const dockRect = hudRefs.dock.getBoundingClientRect();
      if (dockRect && dockRect.height > 0 && dockRect.top > viewTop) {
        bottomLimit = Math.min(bottomLimit, dockRect.top - 10);
      }
    }

    // Position 4 arrows around the building footprint
    const arrows = placement.arrows;
    const refs = hudRefs && hudRefs.placementArrows;
    if (arrows && refs) {
      const minX = viewLeft + 20;
      const maxX = viewLeft + vw - 20;
      const minY = viewTop + 20;
      const maxY = viewTop + vh - 20;
      refs.up.style.left = clamp(arrows.up[0], minX, maxX).toFixed(1) + "px";
      refs.up.style.top = clamp(arrows.up[1], minY, maxY).toFixed(1) + "px";
      refs.right.style.left =
        clamp(arrows.right[0], minX, maxX).toFixed(1) + "px";
      refs.right.style.top =
        clamp(arrows.right[1], minY, maxY).toFixed(1) + "px";
      refs.down.style.left =
        clamp(arrows.down[0], minX, maxX).toFixed(1) + "px";
      refs.down.style.top = clamp(arrows.down[1], minY, maxY).toFixed(1) + "px";
      refs.left.style.left =
        clamp(arrows.left[0], minX, maxX).toFixed(1) + "px";
      refs.left.style.top = clamp(arrows.left[1], minY, maxY).toFixed(1) + "px";
    }

    // Position the action card directly below the down arrow
    if (!panel) return;

    const downArrowY = arrows ? arrows.down[1] : anchorY + 48;
    const upArrowY = arrows ? arrows.up[1] : anchorY - 48;
    const gap = 14;
    const belowCenterY = downArrowY + gap + halfH;
    const aboveCenterY = upArrowY - gap - halfH;
    const placeBelow =
      belowCenterY + halfH <= bottomLimit ||
      aboveCenterY - halfH < viewTop + 64;
    const rawCenterY = placeBelow ? belowCenterY : aboveCenterY;
    const minX = viewLeft + halfW + 10;
    const maxX = viewLeft + vw - halfW - 10;
    const minY = viewTop + halfH + 10;
    const maxY = Math.min(viewTop + vh - halfH - 10, bottomLimit - halfH);
    const left = clamp(anchorX, minX, maxX);
    const top = clamp(rawCenterY, minY, maxY);
    panel.style.left = left.toFixed(1) + "px";
    panel.style.top = top.toFixed(1) + "px";
    panel.dataset.placement = placeBelow ? "below" : "above";
  }

  function renderPlacementPanel(
    placement: HudBuildingPlacement | null | undefined,
  ): boolean {
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
    const tileIdx =
      placement.tile_idx != null ? String(placement.tile_idx) : "0";
    const canConfirm = Boolean(placement.can_confirm);
    const cost = Number(placement.cost || 0);
    const reasonKey = placement.reason || "";
    const locale = window.SOW_LOCALE || "en";
    const renderKey = [
      locale,
      kind,
      tileIdx,
      canConfirm ? "1" : "0",
      String(cost),
      reasonKey,
    ].join(":");

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
          ? '<span class="sow-hud__placement-panel-reason">' +
            escapeHudText(reason) +
            "</span>"
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

  const activeProgressBars: Map<string, HTMLElement> = new Map();
  const activeCelebrations: Set<string> = new Set();

  function renderWorldBuildingProgress(
    items: HudBuildingProgressItem[] | null | undefined,
    openPortTile?: number,
  ): void {
    const overlay = hudRefs && hudRefs.worldProgressOverlay;
    if (!overlay) return;

    if (!items || items.length === 0) {
      if (activeProgressBars.size > 0) {
        overlay.replaceChildren();
        activeProgressBars.clear();
      }
      return;
    }

    const currentBars = new Set<string>();
    const visibleTiles = new Set<number>();
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      if (item.kind === "Warship" && item.tile_idx === openPortTile) continue;
      const levelNum = Math.max(1, item.level);
      const key = item.tile_idx + ":" + item.kind;
      currentBars.add(key);
      let bar = activeProgressBars.get(key);
      if (!bar) {
        bar = document.createElement("div");
        bar.className = "sow-hud__world-building-bar";
        bar.dataset.tile = String(item.tile_idx);
        bar.dataset.kind = item.kind;
        bar.dataset.level = String(levelNum);
        bar.innerHTML =
          '<span class="sow-hud__world-building-thumb" aria-hidden="true">' +
          structureOrUnitIcon(
            item.kind,
            levelNum,
            "sow-hud__world-building-art",
          ) +
          "</span>" +
          '<div class="sow-hud__world-building-track" role="progressbar">' +
          '<div class="sow-hud__world-building-fill" data-building-progress-fill></div>' +
          '<b class="sow-hud__world-building-time" data-building-timer></b>' +
          "</div>";
        overlay.appendChild(bar);
        activeProgressBars.set(key, bar);
      } else if (
        bar.dataset.kind !== item.kind ||
        bar.dataset.level !== String(levelNum)
      ) {
        bar.dataset.kind = item.kind;
        bar.dataset.level = String(levelNum);
        const thumb = bar.querySelector<HTMLElement>(
          ".sow-hud__world-building-thumb",
        );
        if (thumb) {
          thumb.innerHTML = structureOrUnitIcon(
            item.kind,
            levelNum,
            "sow-hud__world-building-art",
          );
        }
      }

      const visible = item.visible !== false;
      const nextDisplay = visible ? "flex" : "none";
      if (bar.style.display !== nextDisplay) {
        bar.style.display = nextDisplay;
      }
      if (visible) {
        const stacked = visibleTiles.has(item.tile_idx);
        bar.classList.toggle("is-stacked", stacked);
        visibleTiles.add(item.tile_idx);
        const nextTransform =
          "translate3d(" +
          item.screen_x.toFixed(1) +
          "px, " +
          item.screen_y.toFixed(1) +
          "px, 0) translate(-50%, 0)";
        if (bar.style.transform !== nextTransform) {
          bar.style.transform = nextTransform;
        }
        patchBuildingProgressBar(bar, item.remaining_seconds, item.progress);
      }
    }

    for (const [key, el] of activeProgressBars.entries()) {
      if (!currentBars.has(key)) {
        el.remove();
        activeProgressBars.delete(key);
      }
    }
  }

  function renderWorldBuildingCelebrations(
    items: HudBuildingCelebrationItem[] | null | undefined,
  ): void {
    const overlay = hudRefs && hudRefs.worldCelebrationsOverlay;
    if (!overlay || !items || items.length === 0) return;

    const isMobileCoarse = Boolean(
      window.matchMedia && window.matchMedia("(hover: none) and (pointer: coarse)").matches,
    );
    const maxConcurrent = isMobileCoarse ? 3 : 8;
    const offsets = isMobileCoarse
      ? [
          [-28, -28, 0],
          [28, -28, 40],
          [0, -44, 80],
        ]
      : [
          [-30, -32, 0],
          [30, -30, 40],
          [-18, -48, 80],
          [18, -46, 60],
          [-38, -14, 100],
          [38, -16, 90],
        ];

    for (let i = 0; i < items.length; i++) {
      if (activeCelebrations.size >= maxConcurrent) break;
      const item = items[i];
      const celebrationKey = item.tile_idx + ":" + item.level;
      if (activeCelebrations.has(celebrationKey) || item.visible === false) {
        continue;
      }
      activeCelebrations.add(celebrationKey);

      const container = document.createElement("div");
      container.className = "sow-hud__world-celebration";
      container.style.transform =
        "translate3d(" +
        item.screen_x.toFixed(1) +
        "px, " +
        item.screen_y.toFixed(1) +
        "px, 0) translate(-50%, -50%)";

      let sparklesHtml = "";
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
        buildingIcon(
          item.kind,
          Math.max(1, item.level),
          "sow-hud__celebration-badge-art",
        ) +
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

  // Slot priorities preserve clockwise order from the top in each view.
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
    send: 4,
    request: 5,
    request_allies: 4,
    emote: 5,
  };
  const RING_BUILD_SLOTS: Record<string, number> = {
    build_city: 0,
    build_factory: 1,
    build_port: 2,
    build_tower: 3,
    build_farm: 4,
    build_sam: 5,
    build_missile_silo: 6,
  };
  const RING_EMOTE_SLOTS: Record<string, number> = {
    greet: 0,
    celebrate: 1,
    threat: 2,
    help: 3,
  };
  const RING_ACCENTS: Record<string, string> = {
    attack: "red",
    nuke: "red",
    spawn: "green",
    alliance: "green",
    send: "green",
    request: "green",
    request_allies: "green",
    fleet: "cyan",
  };
  const RING_HOLD_ACTIONS: Record<string, boolean> = { nuke: true };
  const FACE_EXPRESSIONS: readonly string[] = [
    "happy",
    "calm",
    "fear",
    "angry",
  ];

  function ringSlots(
    mapMenu: HudMapMenu,
    items: HudMapMenuItem[],
    hud: HudRingState,
  ): RingSlot[] {
    const slots: RingSlot[] = [];
    if (mapMenuView === "emote") {
      (hud.emotes || []).forEach(function (definition) {
        const index = RING_EMOTE_SLOTS[definition.kind];
        if (index != null)
          slots[index] = { type: "emote", definition: definition };
      });
      if (hud.emote_active) slots[4] = { type: "clear" };
      slots[5] = { type: "pin", enabled: Boolean(hud.emote_active) };
      return slots.filter(Boolean);
    }
    if (mapMenuView === "build") {
      items.forEach(function (item) {
        const index = RING_BUILD_SLOTS[item.action];
        if (index != null)
          slots[index] = { type: "action", item: item, presentation: null };
      });
      return slots.filter(Boolean);
    }
    items.forEach(function (item) {
      if (RING_BUILD_SLOTS[item.action] != null)
        slots[0] = { type: "group", group: "build" };
      else if (item.action === "emote")
        slots[5] = { type: "group", group: "emote" };
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
    return slots.filter(Boolean);
  }

  function ringSlotIdentity(slot: RingSlot): string {
    if (slot.type === "action") return slot.item.action;
    if (slot.type === "group") return "group:" + slot.group;
    if (slot.type === "emote") return "emote:" + slot.definition.kind;
    return slot.type;
  }

  function ringSlotAccent(slot: RingSlot): string {
    if (slot.type === "action")
      return RING_ACCENTS[slot.item.action] || "gold";
    return "gold";
  }

  function ringSlotLabel(slot: RingSlot): string {
    if (slot.type === "action")
      return slot.presentation
        ? SOW_t(slot.presentation.key)
        : mapLabel(slot.item.action);
    if (slot.type === "group")
      return SOW_t(
        slot.group === "emote"
          ? "hud.map_action_emote"
          : "hud.map_action_build",
      );
    if (slot.type === "emote") return SOW_t(slot.definition.menu_key);
    return SOW_t(slot.type === "clear" ? "hud.emote_clear" : "hud.emote_pin");
  }

  function helpEmoteCooldownSeconds(): number {
    const secs = Number(hudState?.hud?.help_cooldown_seconds || 0);
    return Number.isFinite(secs) && secs > 0 ? Math.ceil(secs) : 0;
  }

  function ringSlotDisabled(slot: RingSlot): boolean {
    if (slot.type === "pin") return !slot.enabled;
    if (slot.type === "emote" && slot.definition.kind === "help") {
      return helpEmoteCooldownSeconds() > 0;
    }
    return Boolean(
      slot.type === "action" &&
      (slot.item.disabled || (slot.presentation && slot.presentation.disabled)),
    );
  }

  function ringSlotReason(
    slot: RingSlot,
    availableGold: number | undefined,
  ): string {
    if (slot.type === "action") return mapActionReason(slot.item, availableGold);
    if (slot.type === "emote" && slot.definition.kind === "help") {
      const cooldown = helpEmoteCooldownSeconds();
      if (cooldown > 0) {
        return tWithFallback(
          "hud.emote_help_cooldown",
          { seconds: String(cooldown) },
          cooldown + "s",
        );
      }
    }
    return "";
  }

  function ringSlotDetail(
    slot: RingSlot,
    availableGold: number | undefined,
  ): string {
    if (slot.type !== "action" && slot.type !== "emote") return "";
    const reason = ringSlotReason(slot, availableGold);
    if (reason) return escapeHudText(reason);
    if (slot.type !== "action") return "";
    const parts = [];
    if (slot.item.level != null)
      parts.push("Lv " + slot.item.level + " → " + (slot.item.level + 1));
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
      (50 + Math.sin(radians) * radius).toFixed(2) +
      " " +
      (50 - Math.cos(radians) * radius).toFixed(2)
    );
  }

  function ringSectorMarkup(slot: RingSlot, index: number, span: number): string {
    const largeArc = span > 180 ? "1" : "0";
    const start = index * span - span / 2 + 1.6;
    const end = index * span + span / 2 - 1.6;
    const wedge =
      "M" +
      ringPoint(start, RING_OUTER_RADIUS) +
      " A" +
      RING_OUTER_RADIUS +
      " " +
      RING_OUTER_RADIUS +
      " 0 " +
      largeArc +
      " 1 " +
      ringPoint(end, RING_OUTER_RADIUS) +
      " L" +
      ringPoint(end, RING_INNER_RADIUS) +
      " A" +
      RING_INNER_RADIUS +
      " " +
      RING_INNER_RADIUS +
      " 0 " +
      largeArc +
      " 0 " +
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
      " 0 " +
      largeArc +
      " 1 " +
      ringPoint(end - 1, rimRadius);
    return (
      '<g class="sow-hud__ring-sector" data-slot="' +
      index +
      '" data-accent="' +
      ringSlotAccent(slot) +
      '">' +
      '<path class="sow-hud__ring-wedge" d="' +
      wedge +
      '"/>' +
      '<path class="sow-hud__ring-rim" d="' +
      rim +
      '" pathLength="1"/><path class="sow-hud__ring-hold" d="' +
      rim +
      '" pathLength="1"/>' +
      "</g>"
    );
  }

  function ringFace(expression: string): HTMLElement {
    const slug = hudState?.hud?.player_art_slug;
    const face = document.createElement("span");
    face.className = "sow-hud__ring-face";
    face.innerHTML = hudIcon("ring_emote", "sow-hud__ring-icon");
    if (!FACE_EXPRESSIONS.includes(expression) || !slug) return face;
    const image = document.createElement("img");
    image.alt = "";
    image.onerror = function () {
      this.remove();
    };
    setAvatarImage(
      image,
      avatarAssetUrl(avatarPortraitPath(slug, expression as AvatarExpression)),
    );
    face.appendChild(image);
    return face;
  }

  function ringSlotIcon(slot: RingSlot, mapMenu: HudMapMenu): HTMLElement {
    if (slot.type === "emote") return ringFace(slot.definition.expression);
    if (slot.type === "clear") return ringFace("calm");
    const title = document.createElement("span");
    if (slot.type === "pin")
      title.innerHTML = hudIcon("ring_pin", "sow-hud__ring-icon");
    else if (slot.type === "group")
      title.innerHTML = hudIcon(
        slot.group === "emote" ? "ring_emote" : "ring_build",
        "sow-hud__ring-icon",
      );
    else {
      const action = slot.item.action;
      const icon =
        slot.presentation?.icon || mapActionLabels[action]?.icon || "plus";
      let building =
        RING_BUILD_SLOTS[action] != null
          ? buildingIcon(
              action.slice(6),
              1,
              "sow-hud__building-icon sow-hud__ring-icon",
            )
          : "";
      if (action === "upgrade_structure" && mapMenu.building)
        building = buildingIcon(
          mapMenu.building.kind,
          mapMenu.building.level,
          "sow-hud__building-icon sow-hud__ring-icon",
        );
      title.innerHTML = building || hudIcon(icon, "sow-hud__ring-icon");
    }
    return title;
  }

  function ringSlotButton(
    slot: RingSlot,
    index: number,
    span: number,
    mapMenu: HudMapMenu,
    hud: HudRingState,
  ): HTMLButtonElement {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "sow-hud__ring-slot";
    button.dataset.slot = String(index);
    button.dataset.accent = ringSlotAccent(slot);
    button.setAttribute("role", "menuitem");
    const angle = (index * span * Math.PI) / 180;
    button.style.left =
      (50 + Math.sin(angle) * RING_ICON_RADIUS).toFixed(2) + "%";
    button.style.top =
      (50 - Math.cos(angle) * RING_ICON_RADIUS).toFixed(2) + "%";
    button.style.setProperty("--slot-angle", index * span + "deg");
    button.style.setProperty("--slot-index", String(index));
    if (slot.type === "action") {
      button.dataset.mapAction = slot.item.action;
      if (RING_HOLD_ACTIONS[slot.item.action]) button.dataset.ringHold = "true";
      if (slot.presentation && slot.presentation.state)
        button.dataset.allianceState = slot.presentation.state;
    } else if (slot.type === "group") button.dataset.mapGroup = slot.group;
    else if (slot.type === "emote")
      button.dataset.emoteKind = slot.definition.kind;
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
      } else hub.innerHTML = hudIcon("home", "sow-hud__ring-icon");
      const allianceStatus = document.createElement("span");
      allianceStatus.className = "sow-hud__ring-hub-status";
      allianceStatus.setAttribute("role", "timer");
      allianceStatus.hidden = true;
      allianceStatus.append(
        document.createElement("strong"),
        document.createElement("small"),
      );
      hub.appendChild(allianceStatus);
    } else {
      hub = document.createElement("button");
      hub.type = "button";
      hub.dataset.mapBack = "true";
      hub.setAttribute("aria-label", SOW_t("hud.map_action_back"));
      hub.innerHTML =
        hudIcon(
          mapMenuView === "emote" ? "ring_emote" : "ring_build",
          "sow-hud__ring-icon",
        ) + '<span class="sow-hud__ring-back" aria-hidden="true"></span>';
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
      title = SOW_t(
        mapMenuView === "emote"
          ? "hud.map_action_emote"
          : "hud.map_action_build",
      );
      detail = "";
    } else {
      const relation =
        mapMenu.relation === "teammate"
          ? ""
          : SOW_t("hud.map_relation_" + (mapMenu.relation || "neutral"));
      title = mapMenu.target_name || relation;
      detail = relation && title !== relation ? escapeHudText(relation) : "";
    }
    readout.dataset.state = slot
      ? ringSlotDisabled(slot)
        ? "blocked"
        : "action"
      : "target";
    const titleNode = readout.querySelector("strong");
    const detailNode = readout.querySelector("small");
    if (!titleNode || !detailNode) return;
    if (titleNode.textContent !== title) titleNode.textContent = title;
    if (detailNode.innerHTML !== detail) detailNode.innerHTML = detail;
    detailNode.hidden = !detail;
  }

  function buildRing(
    menu: HTMLElement,
    slots: RingSlot[],
    mapMenu: HudMapMenu,
    hud: HudRingState,
  ): void {
    menu.replaceChildren();
    menu.dataset.level = mapMenuView;
    menu.style.setProperty("--ring-hold-ms", RING_HOLD_MS + "ms");
    const readout = document.createElement("div");
    readout.className = "sow-hud__ring-readout";
    readout.setAttribute("aria-live", "polite");
    readout.append(
      document.createElement("strong"),
      document.createElement("small"),
    );
    const span = 360 / slots.length;
    const radial = document.createElement("div");
    radial.className = "sow-hud__map-radial";
    radial.innerHTML =
      '<svg class="sow-hud__ring-art" viewBox="0 0 100 100" aria-hidden="true">' +
      '<defs>' +
      '<radialGradient id="sow-ring-sheen" cx="50" cy="50" r="50" gradientUnits="userSpaceOnUse">' +
      '<stop offset="0.38" stop-color="#fff" stop-opacity="0"/>' +
      '<stop offset="0.82" stop-color="#fff" stop-opacity="0.045"/>' +
      '<stop offset="0.96" stop-color="#fff" stop-opacity="0.095"/>' +
      '<stop offset="1" stop-color="#fff" stop-opacity="0"/>' +
      '</radialGradient>' +
      '<radialGradient id="sow-ring-depth" cx="50" cy="44" r="52" gradientUnits="userSpaceOnUse">' +
      '<stop offset="0" stop-color="#1c202a" stop-opacity="0.96"/>' +
      '<stop offset="0.68" stop-color="#0b0d12" stop-opacity="0.96"/>' +
      '<stop offset="1" stop-color="#06070a" stop-opacity="0.98"/>' +
      "</radialGradient>" +
      "</defs>" +
      '<circle class="sow-hud__ring-bezel" cx="50" cy="50" r="49.5"/>' +
      '<circle class="sow-hud__ring-compass" cx="50" cy="50" r="48.2"/>' +
      slots.map((slot, index) => ringSectorMarkup(slot, index, span)).join("") +
      '<circle class="sow-hud__ring-sheen" cx="50" cy="50" r="' +
      RING_OUTER_RADIUS +
      '" fill="url(#sow-ring-sheen)"/>' +
      '<circle class="sow-hud__ring-hub-plate" cx="50" cy="50" r="' +
      RING_HUB_RADIUS +
      '"/>' +
      '<circle class="sow-hud__ring-hub-trim" cx="50" cy="50" r="' +
      (RING_HUB_RADIUS - 1.6).toFixed(1) +
      '"/></svg>';
    slots.forEach(function (slot, index) {
      radial.appendChild(ringSlotButton(slot, index, span, mapMenu, hud));
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
    slots: RingSlot[],
    availableGold: number | undefined,
    targetAvatar: HudAvatarIdentity | null | undefined = undefined,
  ): void {
    const image = menu.querySelector<HTMLImageElement>(
      ".sow-hud__ring-portrait",
    );
    const emblem = menu.querySelector<HTMLElement>(".sow-hud__ring-emblem");
    if (image && emblem) renderAvatarIdentity(image, emblem, targetAvatar);
    const mapMenu: HudMapMenu = hudState?.hud?.map_menu || {};
    const hub = menu.querySelector<HTMLElement>(".sow-hud__ring-hub");
    if (hub) {
      const relation = mapMenu.relation || "neutral";
      if (hub.dataset.relation !== relation) hub.dataset.relation = relation;
      const allianceStatus = hub.querySelector<HTMLElement>(
        ".sow-hud__ring-hub-status",
      );
      if (allianceStatus) {
        const seconds = Number(mapMenu.alliance_remaining_seconds);
        const timed =
          mapMenu.relation === "ally" &&
          Number.isFinite(seconds) &&
          seconds > 0;
        const hidden = !timed;
        if (allianceStatus.hidden !== hidden) allianceStatus.hidden = hidden;
        const title = allianceStatus.querySelector("strong");
        const detail = allianceStatus.querySelector("small");
        const titleText = timed ? formatAllianceCountdown(seconds) : "";
        if (title && title.textContent !== titleText)
          title.textContent = titleText;
        if (detail) {
          const detailText = timed ? SOW_t("hud.map_action_alliance") : "";
          if (detail.textContent !== detailText)
            detail.textContent = detailText;
          const detailHidden = !timed;
          if (detail.hidden !== detailHidden) detail.hidden = detailHidden;
        }
      }
    }
    slots.forEach(function (slot, index) {
      const button = menu.querySelector<HTMLButtonElement>(
        '.sow-hud__ring-slot[data-slot="' + index + '"]',
      );
      if (!button) return;
      const disabled = ringSlotDisabled(slot);
      const name = ringSlotLabel(slot);
      const reason = ringSlotReason(slot, availableGold);
      const accessibleLabel = reason ? name + ". " + reason : name;
      if (button.disabled !== disabled) button.disabled = disabled;
      button.setAttribute("aria-disabled", String(disabled));
      if (button.getAttribute("aria-label") !== accessibleLabel)
        button.setAttribute("aria-label", accessibleLabel);
      let blocked = "";
      if (slot.type === "action" && slot.item.disabled)
        blocked =
          slot.item.reason_seconds != null
            ? "clock"
            : slot.item.reason_key
              ? "lock"
              : "gold";
      else if (slot.type === "emote" && disabled) blocked = "clock";
      if (button.dataset.blocked !== blocked) {
        button.dataset.blocked = blocked;
        const badge = button.querySelector<HTMLElement>(".sow-hud__ring-badge");
        if (badge)
          badge.innerHTML = blocked
            ? hudIcon(blocked, "sow-hud__ring-badge-icon")
            : "";
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
    menu.style.setProperty(
      "--tether-angle",
      Math.atan2(dy, dx).toFixed(4) + "rad",
    );
    menu.style.setProperty(
      "--tether-length",
      Math.max(0, distance - half).toFixed(1) + "px",
    );
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
    const count = ring.slots.length;
    if (!radial || !count) return null;
    const rect = radial.getBoundingClientRect();
    if (!rect.width || !rect.height) return null;
    const dx = ((clientX - rect.left) / rect.width) * 100 - 50;
    const dy = ((clientY - rect.top) / rect.height) * 100 - 50;
    const distance = Math.sqrt(dx * dx + dy * dy);
    if (distance <= RING_HUB_RADIUS) return "hub";
    if (!unbounded && distance > 50) return null;
    const angle = ((Math.atan2(dx, -dy) * 180) / Math.PI + 360) % 360;
    return Math.round(angle / (360 / count)) % count;
  }

  function ringSlotButtonAt(
    menu: HTMLElement,
    index: number,
  ): HTMLButtonElement | null {
    return menu.querySelector<HTMLButtonElement>(
      '.sow-hud__ring-slot[data-slot="' + index + '"]',
    );
  }

  function setRingHot(menu: HTMLElement, index: number): void {
    if (ring.hold && ring.hold.index !== index) ringHoldCancel(menu);
    if (ring.hot === index) return;
    ring.hot = index;
    menu
      .querySelectorAll<HTMLElement>(
        ".sow-hud__ring-sector, .sow-hud__ring-slot",
      )
      .forEach(function (element) {
        element.classList.toggle(
          "is-hot",
          Number(element.dataset.slot) === index,
        );
      });
    renderRingReadout(menu);
  }

  function ringHoldStart(menu: HTMLElement, index: number): void {
    if (ring.hold && ring.hold.index === index) return;
    ringHoldCancel(menu);
    ring.hold = { index: index, started: performance.now() };
    const sector = menu.querySelector<HTMLElement>(
      '.sow-hud__ring-sector[data-slot="' + index + '"]',
    );
    if (sector) sector.classList.add("is-arming");
  }

  function ringHoldCancel(menu: HTMLElement | null): void {
    ring.hold = null;
    if (menu)
      menu
        .querySelectorAll<HTMLElement>(".sow-hud__ring-sector.is-arming")
        .forEach(function (sector) {
          sector.classList.remove("is-arming");
        });
  }

  function ringActivate(
    menu: HTMLElement,
    target: "hub" | number | null,
  ): void {
    if (target === "hub") {
      // The center goes back inside a submenu; at the root it focuses the nameplate and closes.
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
    if (!showRadial) {
      if (ring.session) {
        playUiCue(AudioCue.UiRadialClose);
      }
      menu.classList.add("hidden");
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
    const newSession = ring.session !== session;
    if (newSession) {
      playUiCue(AudioCue.UiRadialOpen);
      ring.session = session;
      menu.dataset.session = String(mapMenu.session);
      menu.dataset.tileIdx = String(mapMenu.tile_idx);
      mapMenuView = "root";
      ring.hot = -1;
      ringHoldCancel(null);
      ring.press = ring.pointerDown
        ? { id: ring.pointerId, fromOpen: true }
        : null;
    }
    const items = mapItems(mapMenu);
    let slots = ringSlots(mapMenu, items, hud);
    if (mapMenuView !== "root" && !slots.length) {
      mapMenuView = "root";
      slots = ringSlots(mapMenu, items, hud);
    }
    menu.classList.toggle("hidden", !slots.length);
    if (!slots.length) {
      if (menu.dataset.renderKey) menu.replaceChildren();
      menu.dataset.renderKey = "";
      delete menu.dataset.level;
      ring.slots = [];
      ring.hot = -1;
      ring.press = null;
      ringHoldCancel(null);
      return false;
    }
    const layout = slots.map(ringSlotIdentity).join(",");
    if (!newSession && layout !== ring.slots.map(ringSlotIdentity).join(",")) {
      ring.hot = -1;
      ring.press = null;
      ringHoldCancel(menu);
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
      layout;
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
    return menu && !menu.classList.contains("hidden") && menu.dataset.level
      ? menu
      : null;
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
    if (button && !button.disabled && button.dataset.ringHold === "true")
      ringHoldStart(menu, index);
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
        if (!menu || !press || press.id !== event.pointerId) {
          if (menu) ringHoldCancel(menu);
          return;
        }
        const target = ringSlotAt(
          menu,
          event.clientX,
          event.clientY,
          press.fromOpen,
        );
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

  function escapeHudText(
    value: string | number | boolean | null | undefined,
  ): string {
    return String(value).replace(/[&<>"']/g, function (character) {
      return (
        { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[
          character
        ] || character
      );
    });
  }

  function devColorHex(value: number[], linear: boolean): string {
    function channel(v: number): string {
      v = Math.max(0, Math.min(1, Number(v) || 0));
      if (linear)
        v = v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055;
      return Math.round(v * 255)
        .toString(16)
        .padStart(2, "0");
    }
    return "#" + channel(value[0]) + channel(value[1]) + channel(value[2]);
  }

  function devColorFromHex(hex: string, linear: boolean): number[] {
    const value = [1, 3, 5].map(function (offset) {
      const v = parseInt(hex.slice(offset, offset + 2), 16) / 255;
      return linear
        ? v <= 0.04045
          ? v / 12.92
          : Math.pow((v + 0.055) / 1.055, 2.4)
        : v;
    });
    return value;
  }

  const DEV_RESET_ICON =
    '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 12a8 8 0 1 0 2.6-5.9"/><path d="M4 4v5h5"/></svg>';
  const DEV_SAVE_ICON =
    '<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>';

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

  function devIsDirty(
    def: HudDevParameter,
    current: boolean | number | number[] | undefined,
  ): boolean {
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
        const range = control.querySelector<HTMLInputElement>(
          'input[type="range"]',
        );
        if (range && input.type !== "range") {
          const rangeMin = Number(range.dataset.devMin);
          const rangeMax = Number(range.dataset.devMax);
          const margin = Math.max(
            (rangeMax - rangeMin) * 0.1,
            Number(range.step) || 0.01,
          );
          range.min = String(value < rangeMin ? value - margin : rangeMin);
          range.max = String(value > rangeMax ? value + margin : rangeMax);
        }
        control
          .querySelectorAll<HTMLInputElement>('[data-dev="' + id + '"]')
          .forEach(function (sibling) {
            if (sibling !== input)
              sibling.value =
                sibling.type === "range"
                  ? input.value
                  : devFormat(input.value, decimals);
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
        const rgb = devColorFromHex(
          input.value,
          input.dataset.devLinear === "1",
        );
        vector[0] = rgb[0];
        vector[1] = rgb[1];
        vector[2] = rgb[2];
        control
          .querySelectorAll<HTMLInputElement>("[data-dev-channel]")
          .forEach(function (channel) {
            const channelIndex = Number(channel.dataset.devChannel);
            if (channelIndex < 3) channel.value = String(vector[channelIndex]);
          });
      } else {
        const index = Number(input.dataset.devChannel);
        vector[index] = Number(input.value);
        if (!Number.isFinite(vector[index])) return;
        devReadout(input, decimals);
        const picker =
          control.querySelector<HTMLInputElement>("[data-dev-picker]");
        if (picker && index < 3)
          picker.value = devColorHex(vector, picker.dataset.devLinear === "1");
      }
    }
    send("set_dev_config", { field: id, value: value });
    saveDevOverride(id, value);
  }

  function getStoredDevOverrides(): Record<
    string,
    boolean | number | number[]
  > {
    try {
      const raw = window.localStorage.getItem("sow_dev_overrides");
      return raw ? JSON.parse(raw) : {};
    } catch {
      return {};
    }
  }

  function saveDevOverride(
    id: string,
    value: boolean | number | number[],
  ): void {
    try {
      const overrides = getStoredDevOverrides();
      const devTools = hudState && hudState.hud && hudState.hud.dev_tools;
      const def =
        devTools && devTools.schema
          ? devTools.schema.find(function (item) {
              return item.id === id;
            })
          : undefined;
      if (def && !devIsDirty(def, value)) {
        delete overrides[id];
      } else {
        overrides[id] = value;
      }
      if (Object.keys(overrides).length > 0) {
        window.localStorage.setItem(
          "sow_dev_overrides",
          JSON.stringify(overrides),
        );
      } else {
        window.localStorage.removeItem("sow_dev_overrides");
      }
    } catch {
      /* storage unavailable */
    }
  }

  function removeDevOverride(opts: {
    field?: string | null;
    section?: string | null;
  }): void {
    try {
      if (!opts.field && !opts.section) {
        window.localStorage.removeItem("sow_dev_overrides");
        return;
      }
      const overrides = getStoredDevOverrides();
      if (opts.field) {
        delete overrides[opts.field];
      } else if (opts.section) {
        const devTools = hudState && hudState.hud && hudState.hud.dev_tools;
        if (devTools && devTools.schema) {
          devTools.schema.forEach(function (def) {
            if (def.section === opts.section) {
              delete overrides[def.id];
            }
          });
        }
      }
      if (Object.keys(overrides).length > 0) {
        window.localStorage.setItem(
          "sow_dev_overrides",
          JSON.stringify(overrides),
        );
      } else {
        window.localStorage.removeItem("sow_dev_overrides");
      }
    } catch {
      /* storage unavailable */
    }
  }

  let devOverridesRestored = false;
  function restoreDevOverridesIfNeeded(_devTools: CompleteHudDevTools): void {
    if (devOverridesRestored) return;
    const overrides = getStoredDevOverrides();
    const keys = Object.keys(overrides);
    devOverridesRestored = true;
    if (keys.length === 0) return;
    keys.forEach(function (id) {
      const val = overrides[id];
      if (val !== undefined) {
        send("set_dev_config", { field: id, value: val });
      }
    });
  }

  function getDevSettingsExport(): { json: string; count: number } {
    const devTools = hudState && hudState.hud && hudState.hud.dev_tools;
    const schema = (devTools && devTools.schema) || [];
    const config = (devTools && devTools.config) || {};
    const changes: Record<string, boolean | number | number[]> = {};
    let count = 0;

    schema.forEach(function (def) {
      const current = config[def.id];
      if (current !== undefined && devIsDirty(def, current)) {
        changes[def.id] = current;
        count++;
      }
    });

    const payload = {
      version: 1,
      exported_at: new Date().toISOString(),
      changes_count: count,
      changes: changes,
    };

    return { json: JSON.stringify(payload, null, 2), count };
  }

  function downloadDevSettings(): void {
    const exported = getDevSettingsExport();
    const blob = new Blob([exported.json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "sow_dev_settings.json";
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);

    const saveBtn =
      hudRefs &&
      hudRefs.devSidebar &&
      hudRefs.devSidebar.querySelector<HTMLButtonElement>(
        '[data-command="dev_save_settings"]',
      );
    if (saveBtn) {
      saveBtn.classList.add("is-saved");
      window.setTimeout(function () {
        saveBtn.classList.remove("is-saved");
      }, 1200);
    }
  }

  function copyDevSettings(button?: HTMLElement | null): void {
    const exported = getDevSettingsExport();
    const originalText = button ? button.textContent : "";
    const onSuccess = function () {
      if (button) {
        button.textContent = "¡Copiado!";
        window.setTimeout(function () {
          if (button) button.textContent = originalText;
          setDevMenu(false);
        }, 800);
      } else {
        setDevMenu(false);
      }
    };

    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(exported.json).then(onSuccess, function () {
        fallbackCopy(exported.json, onSuccess);
      });
    } else {
      fallbackCopy(exported.json, onSuccess);
    }
  }

  function fallbackCopy(text: string, cb: () => void): void {
    try {
      const textarea = document.createElement("textarea");
      textarea.value = text;
      textarea.style.position = "fixed";
      textarea.style.opacity = "0";
      document.body.appendChild(textarea);
      textarea.focus();
      textarea.select();
      document.execCommand("copy");
      document.body.removeChild(textarea);
      cb();
    } catch {
      setDevMenu(false);
    }
  }

  function disarmDevReset() {
    window.clearTimeout(devResetTimer);
    devResetTimer = 0;
    const button =
      hudRefs &&
      hudRefs.devSidebar &&
      hudRefs.devSidebar.querySelector("#sow-hud-dev-reset-all");
    if (button) button.textContent = "Reset all";
  }

  function setDevMenu(open: boolean): void {
    disarmDevReset();
    const menu =
      hudRefs!.devSidebar.querySelector<HTMLElement>("#sow-hud-dev-menu");
    const trigger = hudRefs!.devSidebar.querySelector<HTMLElement>(
      '[data-command="dev_menu"]',
    );
    if (menu) menu.hidden = !open;
    if (trigger) trigger.setAttribute("aria-expanded", open ? "true" : "false");
  }

  function devReadout(input: HTMLInputElement, decimals: number): void {
    const readout =
      input.parentElement && input.parentElement.querySelector("output");
    if (readout) readout.textContent = devFormat(input.value, decimals);
  }

  function devStepOnce(button: HTMLButtonElement): void {
    const control = button.closest<HTMLElement>("[data-dev-control]");
    const range =
      control && control.querySelector<HTMLInputElement>('input[type="range"]');
    const chip =
      control &&
      control.querySelector<HTMLInputElement>('input[type="text"][data-dev]');
    if (!range || !chip) return;
    const next =
      Number(range.value) +
      Number(button.dataset.devStepDir) * (Number(range.step) || 0.01);
    if (!Number.isFinite(next)) return;
    chip.value = String(Number(next.toFixed(6)));
    sendDevControl(chip);
    chip.value = devFormat(next, Number(control.dataset.devDecimals) || 0);
  }

  function devControlMarkup(def: HudDevParameter): string {
    const id = escapeHudText(def.id);
    const label = escapeHudText(def.label);
    const decimals =
      def.kind === "color" || def.kind === "vector" ? 2 : devDecimals(def.step);
    const attrs =
      ' data-dev-control="' +
      id +
      '" data-dev-kind="' +
      def.kind +
      '" data-dev-decimals="' +
      decimals +
      '" data-dev-search-text="' +
      escapeHudText(
        (def.section + " " + def.label + " " + def.id).toLowerCase(),
      ) +
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
    let html =
      '<div class="sow-hud__dev-control"' +
      attrs +
      '><div class="sow-hud__dev-head">' +
      name;
    if (def.kind === "number") {
      const unit = ({ s: "s", "screen px": "px", "%": "%" } as Record<string, string>)[
        def.unit
      ];
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

  function buildDevPanel(
    devTools: CompleteHudDevTools,
    sidebar: HTMLElement,
    host: HTMLElement,
  ): void {
    const bySection: Record<
      string,
      NonNullable<HudDevTools["schema"]>
    > = Object.create(null);
    devTools.schema.forEach(function (def) {
      (bySection[def.section] || (bySection[def.section] = [])).push(def);
    });
    devControlIndex = Object.create(null);
    let options = "";
    let html = "";
    devTools.groups.forEach(function (group) {
      const groupName = escapeHudText(group.name);
      options +=
        '<option value="' +
        groupName +
        '" data-dev-tab="' +
        groupName +
        '">' +
        groupName +
        "</option>";
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
      options += '<option value="Perf" data-dev-tab="Perf">Perf</option>';
    }
    const select = sidebar.querySelector<HTMLSelectElement>(
      "#sow-hud-dev-category-select",
    );
    if (select) select.innerHTML = options;
    host.innerHTML = html;
    host
      .querySelectorAll<HTMLElement>("[data-dev-control]")
      .forEach(function (control) {
        const id = control.dataset.devControl;
        if (id) devControlIndex[id] = control;
      });
  }

  function renderDevControls(devTools: Partial<HudDevTools>): void {
    const sidebar = hudRefs && hudRefs.devSidebar;
    const host =
      sidebar && sidebar.querySelector<HTMLElement>("#sow-hud-dev-controls");
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
      (Array.isArray(completeDevTools.perf) && completeDevTools.perf.length
        ? "#perf"
        : "");
    if (key !== devToolsSchemaKey) {
      buildDevPanel(completeDevTools, sidebar, host);
      devToolsSchemaKey = key;
    }
    restoreDevOverridesIfNeeded(completeDevTools);
    const perfHost = sidebar.querySelector<HTMLElement>("#sow-hud-dev-perf");
    const perf = Array.isArray(completeDevTools.perf)
      ? completeDevTools.perf
      : [];
    if (perfHost) {
      if (
        perfHost.children.length !== perf.length ||
        perf.some(function (item, index) {
          return (
            perfHost.querySelectorAll<HTMLButtonElement>(
              "button[data-dev-perf]",
            )[index]?.dataset.devPerf !== item.name
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
          if (button.dataset.state !== item.state)
            button.dataset.state = item.state;
          button.setAttribute(
            "aria-pressed",
            item.state === "on"
              ? "true"
              : item.state === "mixed"
                ? "mixed"
                : "false",
          );
        });
    }
    const categorySelect = sidebar.querySelector<HTMLSelectElement>(
      "#sow-hud-dev-category-select",
    );
    const options = categorySelect ? Array.from(categorySelect.options) : [];
    if (
      options.length &&
      !options.some(function (opt) {
        return opt.value === devTab;
      })
    ) {
      devTab = options[0]?.value || "";
      try {
        const saved = window.localStorage.getItem("sow_dev_tab");
        if (
          saved &&
          options.some(function (opt) {
            return opt.value === saved;
          })
        )
          devTab = saved;
      } catch {
        /* storage unavailable */
      }
    }
    if (categorySelect && categorySelect.value !== devTab) {
      categorySelect.value = devTab;
    }
    const search = sidebar.querySelector<HTMLInputElement>("[data-dev-search]");
    const query = search ? search.value.trim().toLowerCase() : "";
    const perfTab = devTab === "Perf" && query === "";
    if (perfHost) perfHost.hidden = !perfTab;
    const config = completeDevTools.config || {};
    const dirtyBySection: Record<string, number> = Object.create(null);
    completeDevTools.schema.forEach(function (def) {
      const control = devControlIndex[def.id];
      if (!control) return;
      const current = config[def.id];
      const dirty = devIsDirty(def, current);
      if (dirty)
        dirtyBySection[def.section] = (dirtyBySection[def.section] || 0) + 1;
      const flag = dirty ? "true" : "false";
      if (control.dataset.dirty !== flag) control.dataset.dirty = flag;
      if (def.id.indexOf("water.") === 0 && /_limit$/.test(def.id)) {
        control.dataset.warn =
          Number(current) > Number(config["water.depth_cap"])
            ? "true"
            : "false";
      }
      if (def.kind === "number") {
        const range = control.querySelector<HTMLInputElement>(
          'input[type="range"]',
        );
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
          const margin = Math.max(
            (max - min) * 0.1,
            Number(range.step) || 0.01,
          );
          range.min = String(value < min ? value - margin : min);
          range.max = String(value > max ? value + margin : max);
        }
      }
    });
    const dirtyByGroup: Record<string, boolean> = Object.create(null);
    host
      .querySelectorAll<HTMLDetailsElement>(".sow-hud__dev-section")
      .forEach(function (section) {
        const sectionName = section.dataset.devName;
        const groupName = section.dataset.devGroup;
        if (!sectionName || !groupName) return;
        const count = dirtyBySection[sectionName] || 0;
        section.dataset.dirty = count ? "true" : "false";
        if (count) dirtyByGroup[groupName] = true;
        let matches = 0;
        section
          .querySelectorAll<HTMLElement>("[data-dev-control]")
          .forEach(function (control) {
            const hide =
              query !== "" && control.dataset.devSearchText!.indexOf(query) < 0;
            control.hidden = hide;
            if (!hide) matches++;
          });
        section.hidden =
          query !== ""
            ? matches === 0
            : perfTab || section.dataset.devGroup !== devTab;
        if (query && matches) section.open = true;
      });
    dirtyByGroup.Perf = perf.some(function (item) {
      return item.state !== "on";
    });
    options.forEach(function (option) {
      const tabName = option.value;
      const isDirty = Boolean(dirtyByGroup[tabName]);
      option.dataset.dirty = isDirty ? "true" : "false";
      option.textContent = tabName + (isDirty ? " •" : "");
    });
    const currentDirty = Boolean(dirtyByGroup[devTab]);
    const dot = sidebar.querySelector<HTMLElement>("#sow-hud-dev-category-dot");
    if (dot) dot.hidden = !currentDirty;
    const field = sidebar.querySelector<HTMLElement>(
      ".sow-hud__dev-category-field",
    );
    if (field) field.dataset.dirty = currentDirty ? "true" : "false";
    const preview = completeDevTools.preview || {
      target: null,
      phase: 0,
      paused: false,
      repeat: false,
      active: false,
      playing: false,
    };
    const transport = sidebar.querySelector<HTMLElement>(
      "#sow-hud-dev-transport",
    );
    if (transport) transport.hidden = !preview.active;
    sidebar
      .querySelectorAll<HTMLInputElement>("[data-dev-preview]")
      .forEach(function (input) {
        if (document.activeElement === input) return;
        const field = input.dataset.devPreview;
        if (!field) return;
        const previewValues: Record<string, string | number | boolean | null> =
          {
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
    sidebar
      .querySelectorAll<HTMLButtonElement>(".sow-hud__dev-play")
      .forEach(function (button) {
        button.classList.toggle(
          "is-playing",
          Boolean(preview.active) &&
            Number(button.dataset.devTarget) === preview.target,
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
    return hudIcon("ring_send", cls || "sow-hud__btn-icon");
  }

  function leaderboardGpsIcon(cls?: string): string {
    return hudIcon("home", cls || "sow-hud__btn-icon");
  }

  function leaderboardPlayerAvatar(
    player: HudLeaderboardPlayer,
  ): HudAvatarIdentity {
    return {
      kind: "portrait",
      slug: player.avatar_slug || "null",
      expression: player.avatar_expression || undefined,
      transition: player.avatar_transition,
    };
  }

  function avatarTransitionKey(
    transition: HudAvatarTransition | null | undefined,
  ): string {
    return transition
      ? transition.id +
          ":" +
          transition.from +
          ":" +
          transition.to +
          ":" +
          transition.stage
      : "";
  }

  function updateLeaderboard(
    players: HudLeaderboardPlayer[],
    teams: HudLeaderboardTeam[],
  ): void {
    if (!hudRefs || !hudRefs.rows) return;
    lastLeaderboardTeams = teams;

    const hasTeams =
      (teams && teams.length > 0) ||
      (players || []).some((p) => Boolean(p.team));
    if (hudRefs.lbTeamWrap) {
      hudRefs.lbTeamWrap.hidden = !hasTeams;
      if (!hasTeams && leaderboardTeamFilter !== "all") {
        leaderboardTeamFilter = "all";
        const teamInput = hudRefs.lbTeamWrap.querySelector<HTMLInputElement>(
          "input[name='lb_team']",
        );
        if (teamInput) teamInput.value = "all";
        const teamDropdown = hudRefs.lbTeamWrap.querySelector<HTMLElement>(
          ".sow-control-dropdown",
        );
        if (teamDropdown) teamDropdown.classList.remove("is-filtered");
      }
    }

    const teamPositions = new Map<string, number>();
    teams.forEach(function (team, idx) {
      teamPositions.set(String(team.team), idx);
    });

    const is1v1 = (players || []).filter((p) => !p.is_bot).length === 2;
    leaderboardIs1v1 = is1v1;

    const sortedPlayers = [...(players || [])].sort((a, b) =>
      compareLeaderboardPlayers(a, b, leaderboardSortMode, is1v1),
    );

    const playerPositions = new Map<string, number>();
    sortedPlayers.forEach(function (player, idx) {
      playerPositions.set(String(player.id), idx);
    });
    const teamRenderKey = teams
      .map(function (team, idx) {
        return [
          idx,
          team.team,
          team.tile_count,
          team.territory_pct,
          team.members,
        ].join("|");
      })
      .join("\u001e");
    const playerRenderKey = sortedPlayers
      .map(function (player, idx) {
        return [
          idx,
          player.id,
          player.rank,
          player.name,
          player.troops,
          player.gold,
          player.troop_rate,
          player.gold_rate,
          player.kills,
          player.tile_count,
          player.territory_pct,
          player.is_alive,
          player.is_me,
          player.is_bot,
          player.team,
          player.can_trade,
          player.leader,
          player.avatar,
          player.avatar_expression,
          avatarTransitionKey(player.avatar_transition),
        ].join("|");
      })
      .join("\u001e");
    const renderKey =
      leaderboardSortMode +
      "\u001c" +
      (is1v1 ? "1v1" : "norm") +
      "\u001c" +
      teamRenderKey +
      "\u001d" +
      playerRenderKey;
    const expectedNonEmpty = sortedPlayers.length > 0 || teams.length > 0;
    const rowsPopulatedOrPending =
      !expectedNonEmpty ||
      hudRefs.rows.childElementCount > 0 ||
      leaderboardWork !== null;
    if (renderKey === leaderboardRenderKey && rowsPopulatedOrPending) return;
    leaderboardRenderKey = renderKey;
    leaderboardWork = {
      players: sortedPlayers,
      teams,
      playerPositions,
      teamPositions,
      teamIndex: 0,
      playerIndex: 0,
      staleTeamIds: Object.keys(leaderboardTeamRows).filter(
        (id) => !teamPositions.has(id),
      ),
      staleTeamIndex: 0,
      stalePlayerIds: Object.keys(leaderboardRows).filter(
        (id) => !playerPositions.has(id),
      ),
      stalePlayerIndex: 0,
    };
    scheduleLeaderboardFrame();
  }

  function updateLeaderboardCardFilter(card: HTMLElement): void {
    const q = leaderboardFilterQuery;
    const isTeamSummary = card.classList.contains("sow-hud__team-summary");

    if (leaderboardTeamFilter !== "all") {
      const cardTeam = card.dataset.team || "";
      if (cardTeam !== leaderboardTeamFilter) {
        card.hidden = true;
        return;
      }
    }

    if (!isTeamSummary && leaderboardTypeFilter !== "all") {
      const isBot = card.dataset.isBot === "true";
      if (leaderboardTypeFilter === "humans" && isBot) {
        card.hidden = true;
        return;
      }
      if (leaderboardTypeFilter === "bots" && !isBot) {
        card.hidden = true;
        return;
      }
    }

    const name = (
      card.querySelector(".sow-hud__player-name")?.textContent || ""
    ).toLowerCase();
    const rank = (
      card.querySelector(".sow-hud__player-rank")?.textContent || ""
    ).toLowerCase();
    const matches = !q || name.includes(q) || rank.includes(q);
    card.hidden = !matches;
  }

  function updateLeaderboardEmptyState(): void {
    if (!hudRefs || !hudRefs.lbEmpty) return;
    const hasVisibleRows =
      Object.values(leaderboardRows).some((row) => !row.card.hidden) ||
      Object.values(leaderboardTeamRows).some((row) => !row.card.hidden);
    const isFiltered = Boolean(
      leaderboardFilterQuery ||
      leaderboardTeamFilter !== "all" ||
      leaderboardTypeFilter !== "all",
    );
    hudRefs.lbEmpty.hidden = hasVisibleRows || !isFiltered;
  }

  function applyLeaderboardFilter(): void {
    if (!hudRefs || !hudRefs.rows) return;
    const cards = hudRefs.rows.querySelectorAll<HTMLElement>(
      ".sow-hud__player-card",
    );
    for (const card of Array.from(cards)) updateLeaderboardCardFilter(card);
    updateLeaderboardEmptyState();
  }

  function createLeaderboardTeamRow(): LeaderboardTeamRow {
    const card = document.createElement("div");
    const rank = document.createElement("span");
    const name = document.createElement("b");
    const territory = document.createElement("span");
    card.className = "sow-hud__player-card sow-hud__team-summary";
    rank.className = "sow-hud__player-rank";
    name.className = "sow-hud__player-name";
    territory.className = "sow-hud__player-land";
    card.append(rank, name, territory);
    return { card, rank, name, territory, key: "" };
  }

  function updateLeaderboardTeamRow(
    row: LeaderboardTeamRow,
    team: HudLeaderboardTeam,
    index: number,
  ): void {
    const key = [
      index,
      team.team,
      team.tile_count,
      team.territory_pct,
      team.members,
    ].join("|");
    if (row.key !== key) {
      row.key = key;
      row.rank.textContent = "#" + (index + 1);
      row.name.textContent = SOW_t(
        team.team === "Red" ? "lobbies.red_team" : "lobbies.blue_team",
        {
          count: team.members,
        },
      );
      row.territory.textContent =
        Math.round((team.territory_pct || 0) * 100) + "%";
      row.card.dataset.team = team.team;
      row.card.classList.toggle("is-team-red", team.team === "Red");
      row.card.classList.toggle("is-team-blue", team.team === "Blue");
    }
  }

  function createLeaderboardRow(player: HudLeaderboardPlayer): LeaderboardRow {
    const card = document.createElement("div");
    const left = document.createElement("div");
    const rank = document.createElement("span");
    const avatarWrap = document.createElement("span");
    const avatarImg = document.createElement("img");
    const avatarBadge = document.createElement("span");
    const nameWrap = document.createElement("div");
    const name = document.createElement("b");
    const botBadge = document.createElement("span");
    botBadge.className = "sow-hud__bot-badge";
    botBadge.textContent = "BOT";
    botBadge.hidden = true;
    const duelistBadge = document.createElement("span");
    duelistBadge.className = "sow-hud__duelist-badge";
    duelistBadge.textContent = "1v1";
    duelistBadge.hidden = true;
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
    avatarImg.loading = "lazy";
    avatarImg.decoding = "async";
    bindAvatarFallback(avatarImg, () => leaderboardPanelOpen, true);
    avatarBadge.className = "sow-hud__player-avatar-badge";
    avatarBadge.hidden = true;
    avatarWrap.append(avatarImg, avatarBadge);
    nameWrap.className = "sow-hud__player-name-wrap";
    name.className = "sow-hud__player-name";
    nameWrap.append(name, botBadge, duelistBadge);
    left.append(rank, avatarWrap, nameWrap);
    meta.className = "sow-hud__player-meta";
    stats.className = "sow-hud__player-stats";
    territory.className = "sow-hud__player-land";
    troops.className = "sow-hud__player-troops";
    stats.append(territory, troops);
    actions.className = "sow-hud__player-actions";
    gps.type = "button";
    gps.className =
      "sow-hud__row-action sow-hud__row-action--icon sow-hud__row-action--gps";
    gps.dataset.command = "focus_player";
    gps.dataset.playerId = String(player.id);
    gps.setAttribute("aria-label", SOW_t("hud.center_camera") || "Locate");
    SOWTooltip.set(gps, SOW_t("hud.center_camera") || "Locate");
    gps.innerHTML = leaderboardGpsIcon();
    transfer.type = "button";
    transfer.className =
      "sow-hud__row-action sow-hud__row-action--icon sow-hud__row-action--trade";
    transfer.dataset.command = "open_transfer";
    transfer.dataset.playerId = String(player.id);
    transfer.setAttribute(
      "aria-label",
      SOW_t("hud.map_action_transfer") || "Trade",
    );
    SOWTooltip.set(transfer, SOW_t("hud.map_action_transfer") || "Trade");
    transfer.innerHTML = leaderboardTradeIcon();
    actions.append(gps, transfer);
    meta.append(stats, actions);
    card.append(left, meta);
    return {
      card,
      rank,
      avatarWrap,
      avatarImg,
      avatarBadge,
      name,
      botBadge,
      duelistBadge,
      territory,
      troops,
      gpsBtn: gps,
      transferBtn: transfer,
      key: "",
    };
  }

  function updateLeaderboardRow(
    row: LeaderboardRow,
    player: HudLeaderboardPlayer,
    index: number,
  ): void {
    const displayName = player.name || SOW_t("hud.player_name");
    const rowKey = [
      player.rank,
      index,
      displayName,
      player.troops,
      player.gold,
      player.troop_rate,
      player.gold_rate,
      player.kills,
      leaderboardSortMode,
      player.tile_count,
      player.territory_pct,
      player.is_alive,
      player.is_me,
      player.is_bot,
      player.team,
      leaderboardIs1v1,
      player.can_trade,
      player.leader,
      player.avatar,
      player.avatar_expression,
      avatarTransitionKey(player.avatar_transition),
    ].join("|");
    if (row.key === rowKey) return;
    row.key = rowKey;

    const isBot = !!player.is_bot;
    const isDuelist = leaderboardIs1v1 && !isBot;
    row.card.dataset.team = player.team || "";
    row.card.dataset.isBot = String(isBot);
    row.card.dataset.isHuman = String(!isBot);
    row.card.classList.toggle("is-me", !!player.is_me);
    row.card.classList.toggle("is-dead", !player.is_alive);
    row.card.classList.toggle("is-bot", isBot);
    row.card.classList.toggle("is-duelist", isDuelist);
    if (row.botBadge) row.botBadge.hidden = !isBot;
    if (row.duelistBadge) row.duelistBadge.hidden = !isDuelist;
    if (player.team) {
      row.card.classList.toggle("is-team-red", player.team === "Red");
      row.card.classList.toggle("is-team-blue", player.team === "Blue");
    } else {
      row.card.classList.remove("is-team-red", "is-team-blue");
    }

    const displayRank = index + 1;
    row.card.classList.toggle("is-top-1", displayRank === 1);
    row.card.classList.toggle("is-top-2", displayRank === 2);
    row.card.classList.toggle("is-top-3", displayRank === 3);
    row.rank.textContent = "#" + displayRank;
    row.name.textContent = displayName;
    renderAvatarIdentity(
      row.avatarImg,
      row.avatarBadge,
      leaderboardPlayerAvatar(player),
      true,
    );
    row.territory.textContent =
      Math.round((player.territory_pct || 0) * 100) + "%";
    let statText =
      player.troops > 1000
        ? (player.troops / 1000).toFixed(1) + "k"
        : String(Math.floor(player.troops));
    let statIcon = hudIcon("troops", "sow-hud__inline-icon");
    if (leaderboardSortMode === "gold") {
      statText = formatHudMetric(player.gold);
      statIcon = hudIcon("gold", "sow-hud__inline-icon");
    } else if (leaderboardSortMode === "troop_rate") {
      statText = formatHudRate(player.troop_rate);
    } else if (leaderboardSortMode === "gold_rate") {
      statText = formatHudRate(player.gold_rate);
      statIcon = hudIcon("gold", "sow-hud__inline-icon");
    } else if (leaderboardSortMode === "kills") {
      statText = String(player.kills || 0);
      statIcon = hudIcon("attack", "sow-hud__inline-icon");
    }
    row.troops.innerHTML = escapeHudText(statText) + " " + statIcon;
    row.gpsBtn.dataset.playerId = String(player.id);
    row.transferBtn.dataset.playerId = String(player.id);
    row.transferBtn.hidden = !player.can_trade;
    row.transferBtn.disabled = !player.is_alive;
    row.gpsBtn.disabled = !player.is_alive;
  }

  function insertLeaderboardCard(index: number, card: HTMLElement): void {
    if (!hudRefs || !hudRefs.rows) return;
    const before = hudRefs.rows.children.item(index);
    if (before !== card) hudRefs.rows.insertBefore(card, before);
  }

  function scheduleLeaderboardFrame(): void {
    if (!leaderboardWork || leaderboardFrame) return;
    leaderboardFrame = window.requestAnimationFrame(flushLeaderboardFrame);
  }

  function cancelLeaderboardWork(): void {
    if (leaderboardFrame) window.cancelAnimationFrame(leaderboardFrame);
    leaderboardFrame = 0;
    if (leaderboardWork) leaderboardRenderKey = "";
    leaderboardWork = null;
  }

  function closeLeaderboard(): void {
    const wasOpen = leaderboardPanelOpen;
    leaderboardPanelOpen = false;
    cancelLeaderboardWork();
    if (!wasOpen) return;
    leaderboardRenderKey = "";
    lastLeaderboardPlayersPayload = null;
    lastLeaderboardTeamsPayload = null;
    Object.values(leaderboardRows).forEach((row) => {
      avatarIdentityKeys.delete(row.avatarImg);
      clearAvatarPortrait(row.avatarImg);
      row.avatarBadge.hidden = true;
      row.avatarBadge.textContent = "";
      row.key = "";
    });
  }

  function flushLeaderboardFrame(): void {
    leaderboardFrame = 0;
    const work = leaderboardWork;
    if (!work || !hudRefs || !hudRefs.rows || activeHudPanel !== "leaderboard")
      return;
    let budget = 6;

    while (budget > 0 && work.staleTeamIndex < work.staleTeamIds.length) {
      const teamId = work.staleTeamIds[work.staleTeamIndex++];
      const row = leaderboardTeamRows[teamId];
      if (row) {
        row.card.remove();
        delete leaderboardTeamRows[teamId];
      }
      budget--;
    }
    while (budget > 0 && work.teamIndex < work.teams.length) {
      const rowIndex = work.teamIndex++;
      const team = work.teams[rowIndex];
      const key = String(team.team);
      const index = work.teamPositions.get(key) ?? rowIndex;
      const row =
        leaderboardTeamRows[key] ||
        (leaderboardTeamRows[key] = createLeaderboardTeamRow());
      updateLeaderboardTeamRow(row, team, index);
      insertLeaderboardCard(index, row.card);
      updateLeaderboardCardFilter(row.card);
      budget--;
    }
    while (budget > 0 && work.playerIndex < work.players.length) {
      const index = work.playerIndex++;
      const player = work.players[index];
      const key = String(player.id);
      const playerIndex = work.playerPositions.get(key) ?? index;
      const row =
        leaderboardRows[key] ||
        (leaderboardRows[key] = createLeaderboardRow(player));
      updateLeaderboardRow(row, player, playerIndex);
      insertLeaderboardCard(work.teams.length + playerIndex, row.card);
      updateLeaderboardCardFilter(row.card);
      budget--;
    }
    while (budget > 0 && work.stalePlayerIndex < work.stalePlayerIds.length) {
      const playerId = work.stalePlayerIds[work.stalePlayerIndex++];
      const row = leaderboardRows[playerId];
      if (row) {
        row.card.remove();
        delete leaderboardRows[playerId];
      }
      budget--;
    }

    if (
      work.staleTeamIndex >= work.staleTeamIds.length &&
      work.teamIndex >= work.teams.length &&
      work.playerIndex >= work.players.length &&
      work.stalePlayerIndex >= work.stalePlayerIds.length
    ) {
      leaderboardWork = null;
      updateLeaderboardEmptyState();
      return;
    }
    scheduleLeaderboardFrame();
  }

  function createNotificationCard(
    container: HTMLElement,
    contextual: boolean,
  ): HudNotificationCard {
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
    const locate = document.createElement("button");
    locate.type = "button";
    locate.className = "sow-hud__notification-locate";
    locate.hidden = true;
    locate.dataset.command = "focus_world";
    locate.dataset.focusX = "0";
    locate.dataset.focusY = "0";
    locate.dataset.requireOwnedFleet = "true";
    card.append(seal, portraits, copy, locate);
    container.appendChild(card);
    return {
      card: card,
      seal: seal,
      avatars: avatars,
      locate: locate,
      copy: copy,
      renderKey: "",
    };
  }

  function bindAvatarFallback(
    image: HTMLImageElement,
    isActive: () => boolean = () => true,
    deferLoad = false,
  ): void {
    image.onerror = function () {
      if (!isActive()) return;
      const fallback = avatarAssetUrl("gameplay/avatars/null.webp");
      if (this.getAttribute("src") !== fallback) {
        this.hidden = false;
        this.crossOrigin = "anonymous";
        setAvatarImage(this, fallback);
        if (!deferLoad) void loadAvatarImage(fallback).catch(() => undefined);
      } else {
        this.hidden = true;
        this.removeAttribute("src");
      }
    };
  }

  function renderAvatarIdentity(
    image: HTMLImageElement,
    emblem: HTMLElement,
    presentation: HudAvatarIdentity | null | undefined,
    deferLoad = false,
  ): void {
    const reducedMotion = Boolean(
      typeof document !== "undefined" &&
      typeof document.getElementById === "function" &&
      document
        .getElementById("sow-hud")
        ?.classList.contains("is-reduced-motion"),
    );
    const identityKey =
      (presentation ? JSON.stringify(presentation) : "") + ":" + reducedMotion;
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
      presentation.kind === "portrait" &&
      /^[a-z][a-z0-9_]*$/.test(presentation.slug || "")
        ? presentation.slug || "null"
        : "null";
    const src = avatarAssetUrl(
      avatarPortraitPath(slug, presentation.expression),
    );
    const reducedSource = avatarAssetUrl(
      avatarPortraitPath(
        slug,
        presentation.transition
          ? presentation.transition.to
          : presentation.expression,
      ),
    );
    setAvatarPortrait(
      image,
      src,
      presentation.transition,
      reducedMotion,
      reducedSource,
      deferLoad,
    );
    image.hidden = false;
  }

  function clearNotificationCard(parts: HudNotificationCard): void {
    parts.card.hidden = true;
    parts.renderKey = "";
    parts.seal.innerHTML = "";
    parts.locate.hidden = true;
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
  function withAbilityName(
    values: Record<string, string>,
  ): Record<string, string> {
    if (!values.ability_key) return values;
    return Object.assign({}, values, {
      ability: SOW_t(values.ability_key, {}),
    });
  }

  function notificationIcon(
    key: string,
    values: Record<string, string>,
  ): string {
    // A notice that names the hero whose ability fired shows that ability's icon, keyed by the hero's art slug.
    if (values.hero_art) {
      return abilityIconMarkup(
        values.hero_art,
        "sow-hud__notification-ability",
        "draggable='false'",
      );
    }
    if (key.indexOf("nuke") >= 0)
      return hudIcon("ring_nuke", "sow-hud__notification-icon");
    if (key.indexOf("betrayal") >= 0)
      return statusIcon("traitor", "sow-hud__notification-icon");
    if (/elimination/.test(key))
      return statusIcon("death", "sow-hud__notification-icon");
    if (/attack/.test(key))
      return hudIcon("ring_attack", "sow-hud__notification-icon");
    if (key.indexOf("fleet") >= 0 || key === "hud.warship_ready")
      return hudIcon("ring_fleet", "sow-hud__notification-icon");
    if (key.indexOf("alliance") >= 0)
      return hudIcon("ring_alliance", "sow-hud__notification-icon");
    if (key.indexOf("request") >= 0)
      return hudIcon("ring_request", "sow-hud__notification-icon");
    if (key.indexOf("resource") >= 0)
      return hudIcon("ring_send", "sow-hud__notification-icon");
    if (/build|building|gold/.test(key))
      return hudIcon("tools", "sow-hud__notification-icon");
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
    const fleetId = "fleet_id" in entry ? entry.fleet_id : null;
    parts.locate.hidden = contextual || fleetId == null;
    if (!parts.locate.hidden) {
      parts.locate.dataset.fleetId = String(fleetId);
      parts.locate.disabled = !("fleet_available" in entry && entry.fleet_available);
      const label = SOW_t("hud.warship_locate");
      if (parts.locate.textContent !== label) parts.locate.textContent = label;
    }
    parts.card.hidden = false;
    if (parts.renderKey === renderKey) return;
    parts.renderKey = renderKey;
    const wasEntering = parts.card.classList.contains("is-entering");
    parts.card.className =
      "sow-hud__notification sow-hud__notification--premium" +
      (contextual ? " sow-hud__notification--contextual" : "") +
      (item.priority >= 4 ? " sow-hud__notification--urgent" : "") +
      (/resource_(received|sent)|ally_support/.test(key)
        ? " sow-hud__notification--support"
        : "") +
      (wasEntering ? " is-entering" : "");
    parts.seal.innerHTML = notificationIcon(key, entry.values || {});
    const primaryPlayerId =
      !contextual && avatars[0] && avatars[0].player_id != null
        ? Number(avatars[0].player_id)
        : NaN;
    if (Number.isFinite(primaryPlayerId)) {
      parts.card.dataset.command = "focus_player";
      parts.card.dataset.playerId = String(primaryPlayerId);
    } else {
      delete parts.card.dataset.command;
      delete parts.card.dataset.playerId;
    }
    parts.avatars.forEach(function (slot, index) {
      const presentation = avatars[index] || null;
      slot.wrapper.hidden = !presentation;
      slot.wrapper.className =
        "sow-hud__notification-avatar" +
        (index > 0 && avatars[index - 1]
          ? " sow-hud__notification-avatar--overlap"
          : "");
      const slotPlayerId =
        !contextual && presentation && presentation.player_id != null
          ? Number(presentation.player_id)
          : NaN;
      if (Number.isFinite(slotPlayerId)) {
        slot.wrapper.dataset.command = "focus_player";
        slot.wrapper.dataset.playerId = String(slotPlayerId);
      } else {
        delete slot.wrapper.dataset.command;
        delete slot.wrapper.dataset.playerId;
      }
      renderAvatarIdentity(slot.image, slot.emblem, presentation);
    });
    parts.copy.textContent = SOW_t(key, withAbilityName(entry.values || {}));
    const descriptionKey = (entry.values || {}).description_key;
    if (descriptionKey && key === "hud.ability_notice") {
      parts.copy.textContent += " · " + SOW_t(descriptionKey, {});
    }
  }

  function positionMapFeedback(
    entry: HudMapFeedback,
    card: HudNotificationCard,
  ): void {
    const width = Math.max(1, window.innerWidth || 1);
    const height = Math.max(1, window.innerHeight || 1);
    const rect = card.card.getBoundingClientRect
      ? card.card.getBoundingClientRect()
      : null;
    const cardWidth = Math.min(
      rect && rect.width ? rect.width : 320,
      width - 16,
    );
    const cardHeight = rect && rect.height ? rect.height : 52;
    let dockReserve = 0;
    if (width <= 720 && window.getComputedStyle && hudRoot) {
      dockReserve =
        Number.parseFloat(
          window
            .getComputedStyle(hudRoot)
            .getPropertyValue("--sow-dock-reserve"),
        ) || 128;
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

  function writeHudDensity(
    target: HTMLElement | null,
    density: number,
    storyRoot: boolean,
  ): void {
    if (!target) return;
    if (density < 1) {
      if (target.dataset.compact !== "true") target.dataset.compact = "true";
      const values: Record<string, string> = {
        "--sow-hud-density": density.toFixed(4),
        "--sow-hud-hit-target-zoom": (1 / density).toFixed(4),
        "--sow-hud-text-primary-size": (12 / density).toFixed(2) + "px",
        "--sow-hud-text-secondary-size": (11 / density).toFixed(2) + "px",
        "--sow-player-panel-height":
          (storyRoot ? 92 * density : 92).toFixed(2) + "px",
      };
      if (!storyRoot) {
        values["--sow-tutorial-progress-height"] = "24px";
        values["--sow-tutorial-progress-label-size"] =
          (14 / density).toFixed(2) + "px";
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
        if (target.style.getPropertyValue(property))
          target.style.removeProperty(property);
      });
    }
  }

  let compactHudDensityValue: number | null = null;

  function syncCompactHudDensity(hudScreen = cachedHudScreen()) {
    if (!hudRoot) return 1;
    const active = document.activeElement;
    const isEditingHudField =
      active &&
      hudRoot.contains(active) &&
      /^(INPUT|TEXTAREA|SELECT)$/.test(active.tagName || "");
    const density =
      isEditingHudField && compactHudDensityValue !== null
        ? compactHudDensityValue
        : compactHudDensity(hudScreen.width, hudScreen.height);
    compactHudDensityValue = density;
    writeHudDensity(hudRoot, density, false);
    writeHudDensity(document.getElementById("sow-story"), density, true);
    if (density < 1) {
      const operationCap =
        Math.max(0, (hudScreen.height * 0.2) / density).toFixed(1) + "px";
      if (
        hudRoot.style.getPropertyValue(
          "--sow-hud-layout-operations-max-height",
        ) !== operationCap
      ) {
        hudRoot.style.setProperty(
          "--sow-hud-layout-operations-max-height",
          operationCap,
        );
      }
    } else if (
      hudRoot.style.getPropertyValue("--sow-hud-layout-operations-max-height")
    ) {
      hudRoot.style.removeProperty("--sow-hud-layout-operations-max-height");
    }
    return density;
  }

  function clearCompactHudDensity() {
    compactHudDensityValue = null;
    writeHudDensity(hudRoot, 1, false);
    writeHudDensity(document.getElementById("sow-story"), 1, true);
    if (
      hudRoot &&
      hudRoot.style.getPropertyValue("--sow-hud-layout-operations-max-height")
    ) {
      hudRoot.style.removeProperty("--sow-hud-layout-operations-max-height");
    }
    if (
      hudRoot &&
      hudRoot.style.getPropertyValue("--sow-hud-layout-operations-bottom")
    ) {
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
      window.matchMedia &&
      window.matchMedia("(hover: none) and (pointer: coarse)").matches;
    const compact = coarsePointer && density < 1;
    // Measure before writing. The stack rects below are read after the writes they depend on.
    const dockInnerEl =
      hudRefs.dock &&
      hudRefs.dock.querySelector<HTMLElement>(".sow-hud__dock-inner");
    let operationAnchorTop = window.innerHeight;
    [dockInnerEl || hudRefs.dock].forEach(function (element) {
      if (!element || element.hidden || !element.getClientRects().length)
        return;
      if (hudRefs!.dock && hudRefs!.dock.hidden) return;
      operationAnchorTop = Math.min(
        operationAnchorTop,
        element.getBoundingClientRect().top,
      );
    });
    let top = 0;
    let actionTop = 56;
    if (compact) {
      [
        hudRefs.topbar,
        hudRefs.nameplate,
        document.querySelector<HTMLElement>("#sow-story .sow-story__objective"),
      ].forEach(function (element) {
        if (!element || element.hidden || !element.getClientRects().length)
          return;
        top = Math.max(top, element.getBoundingClientRect().bottom);
      });
      if (hudRefs.topbar && hudRefs.topbar.getClientRects().length)
        actionTop = Math.ceil(
          hudRefs.topbar.getBoundingClientRect().bottom + 8,
        );
    }
    const setPropIfChanged = function (property: string, value: string): void {
      if (hudRoot.style.getPropertyValue(property) !== value) {
        hudRoot.style.setProperty(property, value);
      }
    };
    const removePropIfPresent = function (property: string): void {
      if (hudRoot.style.getPropertyValue(property)) {
        hudRoot.style.removeProperty(property);
      }
    };
    if (operationAnchorTop < window.innerHeight) {
      setPropIfChanged(
        "--sow-hud-layout-operations-bottom",
        Math.ceil(window.innerHeight - operationAnchorTop + 6) + "px",
      );
    } else {
      removePropIfPresent("--sow-hud-layout-operations-bottom");
    }
    if (!compact) {
      ["--sow-hud-layout-top-reserve", "--sow-hud-layout-bottom-edge"].forEach(
        function (property) {
          removePropIfPresent(property);
        },
      );
      syncDefaultDockReserve();
      return;
    }
    setPropIfChanged(
      "--sow-hud-layout-top-reserve",
      Math.ceil(top + 8) + "px",
    );
    const dockInner =
      hudRefs.dock &&
      hudRefs.dock.querySelector<HTMLElement>(".sow-hud__dock-inner");
    const safeBottom =
      Number.parseFloat(
        window.getComputedStyle(hudRoot).getPropertyValue("--sow-sab"),
      ) || 0;
    let stackTop = window.innerHeight;
    let stackBottom = 0;
    [dockInner, hudRefs.operations].forEach(function (element) {
      if (!element || element.hidden || !element.getClientRects().length)
        return;
      const rect = element.getBoundingClientRect();
      stackTop = Math.min(stackTop, rect.top);
      stackBottom = Math.max(stackBottom, rect.bottom);
    });
    if (stackBottom === 0) stackBottom = window.innerHeight;
    const stackHeight = Math.max(0, Math.ceil(stackBottom - stackTop));
    const dockReserve =
      stackHeight > 0 ? stackHeight + Math.max(8, safeBottom) + 8 : 0;
    setPropIfChanged("--sow-dock-reserve", dockReserve + "px");
    setPropIfChanged(
      "--sow-hud-layout-bottom-edge",
      Math.floor(stackTop - 8) + "px",
    );
    const freeOperationsHeight = Math.max(
      0,
      operationAnchorTop - Math.max(actionTop, top + 8) - safeBottom - 6,
    );
    const operationCap =
      Math.max(
        0,
        Math.min(window.innerHeight * 0.2, freeOperationsHeight) / density,
      ).toFixed(1) + "px";
    if (
      hudRoot.style.getPropertyValue(
        "--sow-hud-layout-operations-max-height",
      ) !== operationCap
    ) {
      hudRoot.style.setProperty(
        "--sow-hud-layout-operations-max-height",
        operationCap,
      );
    }
    if (
      activeMapFeedback &&
      mapFeedbackCard &&
      hudRefs.mapFeedback &&
      !hudRefs.mapFeedback.hidden
    )
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
      mapFeedbackCard.card.classList.remove(
        "sow-hud__map-feedback-card--visible",
      );
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
    return {
      element: identity,
      portrait: portrait,
      emblem: emblem,
      copy: copy,
      name: name,
    };
  }

  function renderPlayerIdentity(
    parts: PlayerIdentityParts,
    name: string | null | undefined,
    avatar: HudAvatarIdentity | null | undefined,
  ): void {
    const displayName = name || SOW_t("hud.player_name");
    if (parts.name.textContent !== displayName)
      parts.name.textContent = displayName;
    renderAvatarIdentity(
      parts.portrait,
      parts.emblem,
      avatar || { kind: "fallback" },
    );
  }

  function restoreHudFocus(target: HTMLElement | null): void {
    if (target && !target.isConnected && target.id)
      target = document.getElementById(target.id);
    if (!target || !target.isConnected || !target.getClientRects().length)
      target = document.getElementById("blade");
    if (target && typeof target.focus === "function")
      target.focus({ preventScroll: true });
  }

  function tWithFallback(
    key: string,
    vars?: Record<string, string>,
    fallback?: string,
  ): string {
    const text = SOW_t(key, vars);
    if (text === key && fallback) {
      return fallback.replace(/\{(\w+)\}/g, function (_m, name) {
        return vars && vars[name] != null ? vars[name] : "";
      });
    }
    return text;
  }

  function createInboxRequest(
    request: HudInboxRequest,
    variant: "inbox" | "toast" = "inbox",
  ): InboxRequestParts {
    const element = document.createElement("article");
    element.className =
      "sow-hud__request-card" +
      (variant === "toast" ? " sow-hud__request-toast" : "");
    const identity = createPlayerIdentity();
    const headerRow = document.createElement("div");
    headerRow.className = "sow-hud__request-header-row";
    const type = document.createElement("span");
    type.className = "sow-hud__request-type";
    const timer = document.createElement("span");
    timer.className = "sow-hud__request-timer";
    timer.hidden = true;
    headerRow.append(type, timer);
    const resources = document.createElement("div");
    resources.className = "sow-hud__request-resources";
    // A trade's price sits after the swap mark: what they ask ⇄ what they give.
    const swap = document.createElement("span");
    swap.className = "sow-hud__request-swap";
    swap.setAttribute("aria-hidden", "true");
    swap.textContent = "⇄";
    const metric = function (kind: InboxAmount) {
      const element = document.createElement("span");
      element.className =
        "sow-hud__request-metric" + (kind.startsWith("pay_") ? " is-gain" : "");
      if (kind === "pay_gold") resources.appendChild(swap);
      if (kind === "gold" || kind === "pay_gold") {
        const icon = document.createElement("img");
        icon.className = "sow-hud__currency-icon sow-hud__currency-icon--gold";
        icon.src = currencyAsset("gold");
        icon.alt = "";
        element.appendChild(icon);
      } else element.innerHTML = hudIcon("troops", "sow-hud__inline-icon");
      const value = document.createElement("bdi");
      element.appendChild(value);
      resources.appendChild(element);
      return { element: element, value: value };
    };
    const amounts = {
      gold: metric("gold"),
      troops: metric("troops"),
      pay_gold: metric("pay_gold"),
      pay_troops: metric("pay_troops"),
    };
    identity.copy.append(headerRow, resources);
    const actions = document.createElement("div");
    actions.className = "sow-hud__request-actions";
    const buttons = (["accept", "reject"] as const).map(function (action) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "sow-hud__request-" + action;
      actions.appendChild(button);
      return button;
    });
    const timerBar = document.createElement("span");
    timerBar.className = "sow-hud__request-timer-bar";
    timerBar.setAttribute("aria-hidden", "true");
    const progressFill = document.createElement("span");
    progressFill.className = "sow-hud__request-timer-fill";
    timerBar.appendChild(progressFill);
    element.append(identity.element, actions, timerBar);
    return {
      element: element,
      portrait: identity.portrait,
      emblem: identity.emblem,
      copy: identity.copy,
      name: identity.name,
      type: type,
      timer: timer,
      progressFill: progressFill,
      resources: resources,
      amounts: amounts,
      swap: swap,
      accept: buttons[0],
      reject: buttons[1],
      requestKey: requestKey(request),
      patchedSignature: "",
      lastTimerSeconds: -1,
      lastTimerUrgent: null,
      countdownAnchorKey: "",
      countdownAnimAlt: false,
    };
  }

  function getInboxNumberFormat(): Intl.NumberFormat {
    const locale = String(window.SOW_LOCALE || "en");
    if (!cachedIntegerFormatter || cachedIntegerLocale !== locale) {
      cachedIntegerLocale = locale;
      cachedIntegerFormatter = new Intl.NumberFormat(locale, {
        maximumFractionDigits: 0,
      });
    }
    return cachedIntegerFormatter;
  }

  function patchRequestCardCountdown(
    row: InboxRequestParts,
    key: string,
    nowMs: number,
    reducedMotion = isRequestToastReducedMotion(),
  ): void {
    const countdown = readRequestCountdown(key, requestCountdowns, nowMs);
    if (!countdown) {
      if (!row.timer.hidden) row.timer.hidden = true;
      if (row.lastTimerUrgent !== false) {
        row.element.classList.remove("is-expiring");
        row.lastTimerUrgent = false;
      }
      if (row.countdownAnchorKey !== "") {
        row.countdownAnchorKey = "";
        row.progressFill.style.animation = "none";
        row.progressFill.style.transform = "scaleX(1)";
      }
      row.lastTimerSeconds = -1;
      return;
    }
    if (row.lastTimerSeconds !== countdown.remainingSeconds) {
      row.lastTimerSeconds = countdown.remainingSeconds;
      const label = tWithFallback(
        "hud.request_expires_in",
        { seconds: String(countdown.remainingSeconds) },
        countdown.remainingSeconds + "s",
      );
      if (row.timer.textContent !== label) row.timer.textContent = label;
    }
    if (row.timer.hidden) row.timer.hidden = false;
    if (row.lastTimerUrgent !== countdown.urgent) {
      row.lastTimerUrgent = countdown.urgent;
      row.element.classList.toggle("is-expiring", countdown.urgent);
    }
    const record = requestCountdowns.get(key);
    if (reducedMotion || !record) {
      if (row.countdownAnchorKey !== "reduced") {
        row.countdownAnchorKey = "reduced";
        row.progressFill.style.animation = "none";
      }
      const nextScale = "scaleX(" + countdown.progress.toFixed(3) + ")";
      if (row.progressFill.style.transform !== nextScale) {
        row.progressFill.style.transform = nextScale;
      }
      return;
    }
    const anchorKey =
      record.signature + ":" + Math.round(record.expiresAtMs) + ":" + record.ttlMs;
    if (row.countdownAnchorKey !== anchorKey) {
      row.countdownAnchorKey = anchorKey;
      row.countdownAnimAlt = !row.countdownAnimAlt;
      const elapsedMs = Math.max(0, record.ttlMs - countdown.remainingMs);
      const animName = row.countdownAnimAlt
        ? "sow-request-timer-drain-b"
        : "sow-request-timer-drain-a";
      row.progressFill.style.transform = "";
      row.progressFill.style.animation =
        animName + " " + record.ttlMs + "ms linear -" + elapsedMs + "ms 1 forwards";
    }
  }

  function patchInboxRequestCard(
    row: InboxRequestParts,
    request: HudInboxRequest,
    numbers: Intl.NumberFormat,
    nowMs: number,
    reducedMotion = isRequestToastReducedMotion(),
  ): void {
    const key = requestKey(request);
    const locale = String(window.SOW_LOCALE || "en");
    const sig =
      request.kind +
      ":" +
      request.requester_id +
      ":" +
      (request.is_renewal ? "1" : "0") +
      ":" +
      (request.call_id ?? 0) +
      ":" +
      (request.gold ?? 0) +
      ":" +
      (request.troops ?? 0) +
      ":" +
      (request.pay_gold ?? 0) +
      ":" +
      (request.pay_troops ?? 0) +
      ":" +
      (request.active === false ? "0" : "1") +
      ":" +
      (request.name || "") +
      ":" +
      (request.avatar
        ? request.avatar.kind +
          ":" +
          (request.avatar.slug || "") +
          ":" +
          (request.avatar.player_id ?? "") +
          ":" +
          (request.avatar.expression || "") +
          ":" +
          avatarTransitionKey(request.avatar.transition)
        : "") +
      ":" +
      locale;
    if (row.patchedSignature === sig && row.requestKey === key) {
      patchRequestCardCountdown(row, key, nowMs, reducedMotion);
      return;
    }
    row.patchedSignature = sig;
    row.requestKey = key;
    if (row.element.dataset.requestKey !== key) {
      row.element.dataset.requestKey = key;
    }
    const name = request.name || SOW_t("hud.player_name");
    renderPlayerIdentity(row, name, request.avatar);
    const values: Record<string, string> = { name: name };
    let trade = false;
    (["gold", "troops", "pay_gold", "pay_troops"] as const).forEach(
      function (kind) {
        let value = Number(request[kind]);
        value = Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0;
        if (value > 0 && kind.startsWith("pay_")) trade = true;
        values[kind] = numbers.format(value);
        row.amounts[kind].value.textContent = values[kind];
        row.amounts[kind].element.hidden = value === 0;
        row.amounts[kind].element.setAttribute(
          "aria-label",
          SOW_t(kind.endsWith("gold") ? "hud.gold" : "hud.troops") +
            ": " +
            values[kind],
        );
      },
    );
    row.swap.hidden = !trade;
    const isRenewal = request.kind === "alliance" && Boolean(request.is_renewal);
    const isHelp = request.kind === "help";
    const isResources = request.kind === "resources";
    const hasResourceAmounts = values.gold !== "0" || values.troops !== "0";
    row.type.textContent = isHelp
      ? tWithFallback(
          "hud.help_call_title",
          undefined,
          SOW_t("hud.resource_request_title"),
        )
      : isResources
        ? trade
          ? SOW_t("hud.trade_offer_title")
          : SOW_t("hud.resource_request_title")
        : isRenewal
          ? tWithFallback(
              "hud.alliance_renew_prompt",
              undefined,
              SOW_t("hud.alliance_request_title"),
            )
          : SOW_t("hud.alliance_request_title");
    row.element.classList.toggle("is-renewal", isRenewal);
    row.resources.hidden = isHelp ? !hasResourceAmounts : !isResources;
    row.element.setAttribute(
      "aria-label",
      isHelp
        ? tWithFallback(
            "hud.help_call_aria",
            values,
            SOW_t("hud.resource_request", values),
          )
        : SOW_t(
            !isResources
              ? "hud.alliance_request"
              : trade
                ? "hud.trade_offer"
                : "hud.resource_request",
            values,
          ),
    );
    const suffix = isHelp
      ? "_help_call"
      : isResources
        ? "_resource_request"
        : "_alliance";
    row.accept.dataset.command = "accept" + suffix;
    row.accept.dataset.playerId = String(request.requester_id);
    row.reject.dataset.command = "reject" + suffix;
    row.reject.dataset.playerId = String(request.requester_id);
    row.accept.textContent = SOW_t(
      isHelp || (isResources && !trade) ? "hud.send" : "hud.accept",
    );
    row.reject.textContent = SOW_t(
      isHelp || isResources ? "hud.ignore_request" : "hud.reject",
    );
    row.accept.disabled = request.active === false;
    SOWTooltip.set(
      row.accept,
      row.accept.disabled ? SOW_t("hud.action_unavailable") : null,
    );
    row.element.classList.toggle("is-inactive", row.accept.disabled);
    patchRequestCardCountdown(row, key, nowMs, reducedMotion);
  }

  function syncAllRequestCountdowns(): void {
    if (!hudRefs) return;
    const nowMs = Date.now();
    const reducedMotion = isRequestToastReducedMotion();
    if (activeHudPanel === "inbox") {
      for (const key of hudRefs.inboxRequestKeys) {
        const row = hudRefs.inboxRequests[key];
        if (row) patchRequestCardCountdown(row, key, nowMs, reducedMotion);
      }
    }
    for (const card of hudRefs.requestToastCards) {
      patchRequestCardCountdown(card, card.requestKey, nowMs, reducedMotion);
    }
  }

  function stopRequestTimers(): void {
    if (requestToastExpiryTimer) {
      window.clearTimeout(requestToastExpiryTimer);
      requestToastExpiryTimer = null;
    }
    if (requestCountdownTimer) {
      window.clearTimeout(requestCountdownTimer);
      requestCountdownTimer = null;
    }
  }

  function nextCountdownDelayMs(nowMs: number): number | null {
    if (!hudRefs || requestCountdowns.size === 0) return null;
    let minDelay = Infinity;
    const observeKey = function (key: string): void {
      const record = requestCountdowns.get(key);
      if (!record) return;
      const remainingMs = Math.max(0, record.expiresAtMs - nowMs);
      if (remainingMs <= 0) return;
      const rem = remainingMs % 1000;
      const delay = rem > 0 ? rem : 1000;
      if (delay < minDelay) minDelay = delay;
    };
    if (activeHudPanel === "inbox") {
      for (const key of hudRefs.inboxRequestKeys) observeKey(key);
    }
    for (const card of hudRefs.requestToastCards) observeKey(card.requestKey);
    if (!Number.isFinite(minDelay)) return null;
    return Math.max(80, Math.min(1050, Math.ceil(minDelay) + 15));
  }

  function ensureRequestCountdownLoop(): void {
    if (!visibilityListenerBound && typeof document !== "undefined") {
      visibilityListenerBound = true;
      document.addEventListener("visibilitychange", function () {
        if (!document.hidden && hudRefs) {
          syncAllRequestCountdowns();
          ensureRequestCountdownLoop();
        } else if (document.hidden && requestCountdownTimer) {
          window.clearTimeout(requestCountdownTimer);
          requestCountdownTimer = null;
        }
      });
    }
    if (requestCountdownTimer) {
      window.clearTimeout(requestCountdownTimer);
      requestCountdownTimer = null;
    }
    const isHidden = typeof document !== "undefined" && document.hidden;
    const needsTick =
      Boolean(hudRefs) &&
      !isHidden &&
      requestCountdowns.size > 0 &&
      (activeHudPanel === "inbox" || requestToastDeck.entries.length > 0);
    if (!needsTick) return;
    const delayMs = nextCountdownDelayMs(Date.now());
    if (delayMs === null) return;
    requestCountdownTimer = window.setTimeout(function () {
      requestCountdownTimer = null;
      syncAllRequestCountdowns();
      ensureRequestCountdownLoop();
    }, delayMs);
  }

  function isRequestToastReducedMotion(): boolean {
    if (hudRoot && hudRoot.classList.contains("is-reduced-motion")) return true;
    return Boolean(
      typeof window !== "undefined" &&
      window.matchMedia &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    );
  }

  function syncRequestToastsContainerVisibility(
    inboxOpen: boolean,
    visibleCount: number,
  ): void {
    if (!hudRefs || !hudRefs.requestToasts) return;
    const visible = !inboxOpen && (visibleCount > 0 || leavingToastCards.size > 0);
    if (lastRequestToastsVisible !== visible) {
      lastRequestToastsVisible = visible;
      hudRefs.requestToasts.hidden = !visible;
      if (hudRoot) {
        hudRoot.dataset.requestToastsVisible = String(visible);
      }
    }
    if (lastRequestToastsStackCount !== visibleCount) {
      lastRequestToastsStackCount = visibleCount;
      hudRefs.requestToasts.dataset.stackCount = String(visibleCount);
    }
  }

  function finalizeLeavingToastCard(key: string): void {
    const active = leavingToastCards.get(key);
    if (!active) return;
    if (active.timer !== null) {
      window.clearTimeout(active.timer);
    }
    leavingToastCards.delete(key);
    active.element.remove();
    syncRequestToastsContainerVisibility(
      activeHudPanel === "inbox",
      hudRefs ? hudRefs.requestToastCards.length : 0,
    );
  }

  function clearLeavingRequestToasts(): void {
    pendingToastExitModes.clear();
    for (const [key, active] of Array.from(leavingToastCards.entries())) {
      if (active.timer !== null) {
        window.clearTimeout(active.timer);
      }
      leavingToastCards.delete(key);
      active.element.remove();
    }
  }

  function startLeavingToastCard(
    card: RequestToastCardParts,
    mode: RequestToastExitMode,
  ): void {
    finalizeLeavingToastCard(card.requestKey);
    const el = card.element;
    el.classList.remove("is-entering", "is-front", "is-stacked");
    if (activeHudPanel === "inbox" || isRequestToastReducedMotion()) {
      el.remove();
      return;
    }
    el.classList.add("is-leaving", "is-leaving-" + mode);
    el.style.zIndex = String(MAX_TOAST_Z_BASE + 2);
    const onDone = function (event?: AnimationEvent): void {
      if (
        event &&
        (event.target !== el ||
          !event.animationName ||
          event.animationName.indexOf("sow-request-toast-leave-") !== 0)
      ) {
        return;
      }
      el.removeEventListener("animationend", onDone as EventListener);
      finalizeLeavingToastCard(card.requestKey);
    };
    el.addEventListener("animationend", onDone as EventListener);
    const timer = window.setTimeout(function () {
      finalizeLeavingToastCard(card.requestKey);
    }, 320);
    leavingToastCards.set(card.requestKey, { element: el, timer });
  }

  function triggerRequestToastEnterAnimation(el: HTMLElement): void {
    if (isRequestToastReducedMotion()) {
      el.classList.remove("is-entering");
      return;
    }
    el.classList.add("is-entering");
    const onDone = function (event?: AnimationEvent): void {
      if (
        event &&
        (event.target !== el ||
          !event.animationName ||
          event.animationName.indexOf("sow-request-toast-enter") !== 0)
      ) {
        return;
      }
      el.removeEventListener("animationend", onDone as EventListener);
      el.classList.remove("is-entering");
    };
    el.addEventListener("animationend", onDone as EventListener);
    window.setTimeout(function () {
      el.classList.remove("is-entering");
    }, 320);
  }

  function renderRequestToasts(
    requests: HudInboxRequest[],
    inboxOpen: boolean,
    hudScreen: HudScreenSize = cachedHudScreen(),
    force = false,
  ): void {
    if (!hudRefs || !hudRefs.requestToasts || !hudRefs.requestToastStack) return;
    if (
      !force &&
      !inboxOpen &&
      requests.length === 0 &&
      requestToastDeck.entries.length === 0 &&
      hudRefs.requestToastCards.length === 0 &&
      leavingToastCards.size === 0 &&
      lastRequestToastsVisible === false
    ) {
      return;
    }
    const maxVisible = resolveRequestToastMaxVisible(
      hudScreen.width,
      hudScreen.height,
      Boolean(hudRoot && hudRoot.dataset.compact === "true"),
    );
    const inputKey =
      String(window.SOW_LOCALE || "en") +
      ":" +
      requestToastStateVersion +
      ":" +
      (inboxOpen ? "1" : "0") +
      ":" +
      maxVisible;
    if (!force && inputKey === requestToastRenderKey) {
      return;
    }
    requestToastRenderKey = inputKey;

    if (inboxOpen) {
      clearLeavingRequestToasts();
    }
    const nowMs = Date.now();
    const view = reconcileRequestToastDeck(
      requestToastDeck,
      requests,
      requestCountdowns,
      nowMs,
      { inboxOpen: inboxOpen, maxVisible: maxVisible },
    );
    if (requestToastExpiryTimer) {
      window.clearTimeout(requestToastExpiryTimer);
      requestToastExpiryTimer = null;
    }
    if (view.nextExpiryMs != null && view.visible.length > 0) {
      requestToastExpiryTimer = window.setTimeout(
        function () {
          requestToastExpiryTimer = null;
          requestToastStateVersion++;
          renderRequestToasts(latestInboxRequests, activeHudPanel === "inbox");
        },
        Math.max(20, view.nextExpiryMs - nowMs),
      );
    }
    const reducedMotion = isRequestToastReducedMotion();
    const existingByKey = new Map<string, RequestToastCardParts>();
    for (const card of hudRefs.requestToastCards) {
      existingByKey.set(card.requestKey, card);
    }
    const numbers = view.visible.length > 0 ? getInboxNumberFormat() : null;
    const nextCards: RequestToastCardParts[] = [];
    const visibleKeys = new Set<string>();
    view.visible.forEach(function (entry: RequestToastEntry, index: number) {
      visibleKeys.add(entry.key);
      if (leavingToastCards.has(entry.key)) {
        finalizeLeavingToastCard(entry.key);
      }
      let card = existingByKey.get(entry.key);
      const isNew = !card;
      if (!card) {
        card = Object.assign(
          createInboxRequest(entry.request, "toast"),
          { stackIndex: index },
        );
      }
      card.stackIndex = index;
      patchInboxRequestCard(
        card,
        entry.request,
        numbers!,
        nowMs,
        reducedMotion,
      );
      card.element.dataset.stackIndex = String(index);
      card.element.style.setProperty("--stack-i", String(index));
      card.element.classList.toggle("is-front", index === 0);
      card.element.classList.toggle("is-stacked", index > 0);
      if (index > 0) {
        // Clear any leftover entrance lock immediately when pushed back into the 3D deck.
        card.element.classList.remove("is-entering");
      } else if (isNew) {
        triggerRequestToastEnterAnimation(card.element);
      }
      nextCards.push(card);
    });
    for (const [key, oldCard] of existingByKey.entries()) {
      if (!visibleKeys.has(key)) {
        const mode = pendingToastExitModes.get(key) || "expire";
        pendingToastExitModes.delete(key);
        startLeavingToastCard(oldCard, mode);
      }
    }
    // Insert active deck cards back-to-front at indices 0..N-1; leaving cards remain at >= N while flying out.
    nextCards.forEach(function (card, index) {
      card.element.style.zIndex = String(MAX_TOAST_Z_BASE - index);
      const before = hudRefs!.requestToastStack.children[index] || null;
      if (before !== card.element) {
        hudRefs!.requestToastStack.insertBefore(card.element, before);
      }
    });
    hudRefs.requestToastCards = nextCards;
    if (view.overflowCount > 0) {
      hudRefs.requestToastMore.textContent = tWithFallback(
        "hud.request_stack_more",
        { count: String(view.overflowCount) },
        "+" + view.overflowCount,
      );
      hudRefs.requestToastMore.hidden = false;
    } else {
      hudRefs.requestToastMore.hidden = true;
    }
    syncRequestToastsContainerVisibility(inboxOpen, view.visible.length);
    ensureRequestCountdownLoop();
  }

  const MAX_TOAST_Z_BASE = 10;

  function matchesInboxFilter(
    request: HudInboxRequest,
    filter: InboxCategoryFilter,
  ): boolean {
    if (filter === "all") return true;
    if (filter === "alliance") return request.kind === "alliance";
    return request.kind === "resources" || request.kind === "help";
  }

  function inboxRequestIntent(
    request: HudInboxRequest,
    accept: boolean,
  ): string {
    if (request.kind === "alliance") {
      return accept ? "accept_alliance" : "reject_alliance";
    }
    if (request.kind === "resources") {
      return accept ? "accept_resource_request" : "reject_resource_request";
    }
    return accept ? "accept_help_call" : "reject_help_call";
  }

  function renderInbox(requests: HudInboxRequest[]): void {
    if (!hudRefs || !hudRefs.inboxRows || !Array.isArray(requests)) return;
    const renderKey =
      String(window.SOW_LOCALE || "en") +
      ":" +
      inboxFilter +
      ":" +
      inboxDataVersion;
    if (renderKey === hudRefs.inboxRenderKey) return;
    const nowMs = Date.now();
    const reducedMotion = isRequestToastReducedMotion();
    const previous = hudRefs.inboxRequests;
    const previousKeys = hudRefs.inboxRequestKeys;
    const focusedKey = previousKeys.find(function (key) {
      return previous[key].element.contains(document.activeElement);
    });
    const next: Record<string, InboxRequestParts> = Object.create(null);
    const seenKeys = new Set<string>();
    const keys: string[] = [];
    let totalCount = 0;
    let allianceCount = 0;
    let aidCount = 0;
    const numbers = getInboxNumberFormat();
    requests.forEach(function (request) {
      if (!isValidInboxRequest(request)) return;
      const key = requestKey(request);
      if (seenKeys.has(key)) return;
      seenKeys.add(key);
      totalCount += 1;
      if (request.kind === "alliance") {
        allianceCount += 1;
      } else {
        aidCount += 1;
      }
      if (!matchesInboxFilter(request, inboxFilter)) return;
      const row = previous[key] || createInboxRequest(request, "inbox");
      patchInboxRequestCard(row, request, numbers, nowMs, reducedMotion);
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
    const filterCounts: Record<InboxCategoryFilter, number> = {
      all: totalCount,
      alliance: allianceCount,
      aid: aidCount,
    };
    hudRefs.inboxFilterButtons.forEach(function (tab) {
      const tabFilter = tab.dataset.inboxFilter as
        | InboxCategoryFilter
        | undefined;
      if (!tabFilter) return;
      const isSelected = tabFilter === inboxFilter;
      tab.classList.toggle("is-active", isSelected);
      tab.setAttribute("aria-selected", String(isSelected));
      const badge = tab.querySelector<HTMLElement>("[data-filter-count]");
      if (badge) {
        badge.textContent = numbers.format(filterCounts[tabFilter] || 0);
      }
    });
    const hasAcceptable = keys.some(function (key) {
      return !next[key].accept.disabled;
    });
    hudRefs.inboxBulk.hidden = keys.length < 2;
    hudRefs.inboxAcceptAll.disabled = !hasAcceptable;
    hudRefs.inboxRejectAll.disabled = false;
    hudRefs.inboxRows.hidden = keys.length === 0;
    if (hudRefs.inboxEmpty) {
      hudRefs.inboxEmpty.hidden = keys.length !== 0;
      hudRefs.inboxEmpty.textContent =
        totalCount === 0
          ? SOW_t("hud.no_pending_requests")
          : tWithFallback(
              "hud.no_filtered_requests",
              {},
              "No requests in this category.",
            );
    }
    hudRefs.inboxTotal!.textContent = numbers.format(totalCount);
    ensureRequestCountdownLoop();
    if (focusedKey && !next[focusedKey]) {
      const nextRow =
        next[keys[Math.min(previousKeys.indexOf(focusedKey), keys.length - 1)]];
      (nextRow
        ? nextRow.accept.disabled
          ? nextRow.reject
          : nextRow.accept
        : hudRefs.inboxClose
      ).focus();
    }
  }

  function triggerBellAnimation(): void {
    if (!hudRefs || !hudRefs.inboxButton) return;
    const btn = hudRefs.inboxButton;
    if (
      (btn.classList &&
        typeof btn.classList.contains === "function" &&
        btn.classList.contains("is-ringing")) ||
      (typeof bellRingTimer !== "undefined" && bellRingTimer !== null)
    ) {
      return;
    }
    const clearRinging = function (): void {
      if (hudRefs && hudRefs.inboxButton && hudRefs.inboxButton.classList) {
        const activeBtn = hudRefs.inboxButton;
        if (typeof activeBtn.classList.remove === "function") {
          activeBtn.classList.remove("is-ringing");
        } else if (typeof activeBtn.classList.toggle === "function") {
          activeBtn.classList.toggle("is-ringing", false);
        }
      }
      if (typeof bellRingTimer !== "undefined" && bellRingTimer) {
        if (typeof clearTimeout === "function") clearTimeout(bellRingTimer);
        bellRingTimer = null;
      }
    };
    if (btn.classList) {
      if (typeof btn.classList.add === "function") {
        btn.classList.add("is-ringing");
      } else if (typeof btn.classList.toggle === "function") {
        btn.classList.toggle("is-ringing", true);
      }
    }
    if (typeof btn.addEventListener === "function") {
      btn.addEventListener("animationend", clearRinging, { once: true });
    }
    if (typeof setTimeout === "function") {
      const timer = setTimeout(clearRinging, 850) as unknown as number;
      if (typeof bellRingTimer !== "undefined") {
        bellRingTimer = timer;
      }
    }
  }
  if (typeof window !== "undefined") {
    window.SOW_triggerBellAlert = triggerBellAnimation;
  }

  function syncNotificationsVisibleDataset(): void {
    if (!hudRoot) return;
    hudRoot.dataset.notificationsVisible = String(
      notificationCardsByKey.size > 0 || leavingNotificationCards.size > 0,
    );
  }

  function finalizeLeavingNotificationCard(key: string): void {
    const entry = leavingNotificationCards.get(key);
    if (!entry) return;
    if (entry.timer !== null) window.clearTimeout(entry.timer);
    leavingNotificationCards.delete(key);
    clearNotificationCard(entry.parts);
    entry.parts.card.remove();
    syncNotificationsVisibleDataset();
  }

  function clearAllNotificationCards(): void {
    if (notificationTimer) {
      window.clearTimeout(notificationTimer);
      notificationTimer = null;
    }
    leavingNotificationCards.forEach(function (entry) {
      if (entry.timer !== null) window.clearTimeout(entry.timer);
      clearNotificationCard(entry.parts);
      entry.parts.card.remove();
    });
    leavingNotificationCards.clear();
    notificationCardsByKey.forEach(function (parts) {
      clearNotificationCard(parts);
      parts.card.remove();
    });
    notificationCardsByKey.clear();
    notificationCards = [];
    syncNotificationsVisibleDataset();
  }

  function triggerNotificationEnterAnimation(el: HTMLElement): void {
    if (isRequestToastReducedMotion()) {
      el.classList.remove("is-entering");
      return;
    }
    el.classList.remove("is-leaving");
    el.classList.add("is-entering");
    const onDone = function (event?: AnimationEvent): void {
      if (
        event &&
        (event.target !== el ||
          !event.animationName ||
          event.animationName.indexOf("sow-notification-enter") !== 0)
      ) {
        return;
      }
      el.removeEventListener("animationend", onDone as EventListener);
      el.classList.remove("is-entering");
    };
    el.addEventListener("animationend", onDone as EventListener);
    window.setTimeout(function () {
      el.classList.remove("is-entering");
    }, 320);
  }

  function startLeavingNotificationCard(
    key: string,
    parts: HudNotificationCard,
  ): void {
    finalizeLeavingNotificationCard(key);
    const el = parts.card;
    el.classList.remove("is-entering");
    if (isRequestToastReducedMotion()) {
      clearNotificationCard(parts);
      el.remove();
      syncNotificationsVisibleDataset();
      return;
    }
    el.classList.add("is-leaving");
    const onDone = function (event?: AnimationEvent): void {
      if (
        event &&
        (event.target !== el ||
          !event.animationName ||
          event.animationName.indexOf("sow-notification-leave") !== 0)
      ) {
        return;
      }
      el.removeEventListener("animationend", onDone as EventListener);
      finalizeLeavingNotificationCard(key);
    };
    el.addEventListener("animationend", onDone as EventListener);
    const timer = window.setTimeout(function () {
      finalizeLeavingNotificationCard(key);
    }, 340);
    leavingNotificationCards.set(key, { parts, timer });
  }

  function renderNotifications(
    entries: HudNotification[],
    forceRefresh: boolean,
    hudScreen: HudScreenSize = cachedHudScreen(),
  ): void {
    if (!hudRefs || !hudRefs.notifications || !Array.isArray(entries)) return;
    let changed = Boolean(forceRefresh);
    const now = Date.now();
    const compact = window.SOWCampaignView.isCompactViewport(
      hudRoot,
      hudScreen.width,
      hudScreen.height,
    );
    const limit = compact ? 2 : 3;
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
    let hadNewNotification = false;
    entries.forEach(function (entry) {
      const id = Number(entry && entry.id);
      if (!Number.isSafeInteger(id)) return;
      if (id <= notificationCursor) {
        if (entry.fleet_id != null) {
          const active = activeNotifications.find((item) => item.entry.id === id);
          if (active && "fleet_available" in active.entry &&
              active.entry.fleet_available !== entry.fleet_available) {
            active.entry = entry;
            changed = true;
          }
        }
        return;
      }
      notificationCursor = id;
      const age = Math.max(0, Number(entry.age_ms) || 0);
      if (age >= 3600) return;
      hadNewNotification = true;
      const createdAt = Date.now();
      const prio = Math.max(0, Number(entry.priority) || 0);
      playUiCue(AudioCue.UiToast, prio);
      const item = {
        entry: entry,
        priority: prio,
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
              if (Number.isFinite(added))
                totals[key] = (totals[key] || 0) + added;
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
            const resourceSuffix =
              hasGold && hasTroops ? "both" : hasGold ? "gold" : "troops";
            mergedKey = "hud.resource_" + direction + "_" + resourceSuffix;
          }
          item.entry = Object.assign({}, entry, {
            key: mergedKey,
            values: merged,
          });
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
          (current.priority === candidate.priority &&
            current.createdAt < candidate.createdAt)
        )
          replaceIndex = i;
      }
      if (item.priority >= activeNotifications[replaceIndex].priority) {
        activeNotifications[replaceIndex] = item;
        changed = true;
      }
    });
    if (hadNewNotification && typeof triggerBellAnimation === "function") {
      triggerBellAnimation();
    }
    if (!changed) {
      syncNotificationsVisibleDataset();
      return;
    }
    activeNotifications.sort(function (a, b) {
      return (
        b.priority - a.priority ||
        a.createdAt - b.createdAt ||
        Number(a.entry.id) - Number(b.entry.id)
      );
    });
    const container = hudRefs.notifications;
    const activeKeys = new Set<string>();
    activeNotifications.forEach(function (item) {
      activeKeys.add(item.group);
    });
    for (const [key, oldParts] of Array.from(notificationCardsByKey.entries())) {
      if (!activeKeys.has(key)) {
        notificationCardsByKey.delete(key);
        startLeavingNotificationCard(key, oldParts);
      }
    }
    const orderedActiveCards: HudNotificationCard[] = [];
    activeNotifications.forEach(function (item) {
      if (leavingNotificationCards.has(item.group)) {
        finalizeLeavingNotificationCard(item.group);
      }
      let parts = notificationCardsByKey.get(item.group);
      const isNew = !parts;
      if (!parts) {
        parts = createNotificationCard(container, false);
        notificationCardsByKey.set(item.group, parts);
      }
      renderNotificationCard(parts, item, false);
      if (isNew) {
        triggerNotificationEnterAnimation(parts.card);
      }
      orderedActiveCards.push(parts);
    });
    for (let i = orderedActiveCards.length - 1; i >= 0; i -= 1) {
      const card = orderedActiveCards[i].card;
      const nextActive =
        i + 1 < orderedActiveCards.length ? orderedActiveCards[i + 1].card : null;
      if (
        card.parentNode !== container ||
        (nextActive &&
          !(card.compareDocumentPosition(nextActive) & Node.DOCUMENT_POSITION_FOLLOWING))
      ) {
        container.insertBefore(card, nextActive);
      }
    }
    notificationCards = orderedActiveCards;
    syncNotificationsVisibleDataset();
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

  function createCombatOperationRow(kind: HudCombatOperation["kind"]): CombatOperationRow {
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
    glyph.innerHTML = hudIcon(kind === "fleet" ? "ring_fleet" : "ring_attack");
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
    SOWTooltip.set(button, label);
    button.innerHTML =
      command === "scuttle_fleet"
        ? statusIcon("death", "sow-hud__operation-action-icon")
        : hudIcon(glyph, "sow-hud__operation-action-icon");
    if (command === "focus_world") {
      button.dataset.focusX = String(operation.focus_x);
      button.dataset.focusY = String(operation.focus_y);
      const fleetId = operation.focus_fleet_id ??
        (operation.kind === "fleet" && operation.direction === "outgoing"
          ? operation.id
          : null);
      if (fleetId != null) button.dataset.fleetId = String(fleetId);
    } else if (command === "counter_attack") {
      button.dataset.playerId = String(operation.player_id);
      button.dataset.incomingTroops = String(
        operation.incoming_troops || operation.troops || 0,
      );
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
          (incomingTotals[playerId] || 0) +
          Math.max(0, Number(operation.troops) || 0);
      }
    });
    Object.keys(pendingCounterAttacks).forEach(function (playerId) {
      if (
        (incomingTotals[playerId] || 0) <
        pendingCounterAttacks[playerId] - 0.5
      ) {
        delete pendingCounterAttacks[playerId];
      }
    });

    const nextRows: Record<string, CombatOperationRow> = Object.create(null);
    const nextKeys: string[] = [];
    operations.forEach(function (operation) {
      if (
        !operation ||
        (operation.kind !== "attack" && operation.kind !== "fleet")
      )
        return;
      const key = operation.kind + ":" + operation.id;
      nextKeys.push(key);
      const row =
        hudRefs!.combatOperationRows[key] || createCombatOperationRow(operation.kind);
      const incoming = operation.direction === "incoming";
      const outgoing = operation.direction === "outgoing";
      const neutral = Boolean(operation.neutral);
      const retreating = Boolean(operation.retreating);
      const playerId = String(operation.player_id);
      const name = neutral
        ? SOW_t("hud.land")
        : operation.name || SOW_t("hud.player_name");
      const troops = Math.max(0, Number(operation.troops) || 0);
      let details =
        operation.kind === "attack" && incoming
          ? SOW_t("hud.attack_incoming", {
              count: formatNameplateTroops(troops),
            })
          : SOW_t(
              operation.kind === "fleet"
                ? "hud.map_action_fleet"
                : "hud.map_action_attack",
            );
      let detailsEta = "";
      if (operation.kind === "fleet" && outgoing) {
        details = SOW_t(
          retreating ? "hud.returning_transport" : "hud.map_action_fleet",
        );
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
      if (row.detailsLabel.textContent !== details)
        row.detailsLabel.textContent = details;
      const detailsSeparatorText = detailsEta === "" ? "" : " · ";
      if (row.detailsSeparator.textContent !== detailsSeparatorText)
        row.detailsSeparator.textContent = detailsSeparatorText;
      row.detailsSeparator.hidden = detailsEta === "";
      if (row.detailsEta.textContent !== detailsEta)
        row.detailsEta.textContent = detailsEta;
      row.detailsEta.hidden = detailsEta === "";
      const troopText = formatNameplateTroops(troops);
      if (row.troops.textContent !== troopText)
        row.troops.textContent = troopText;
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
      const actionKey = [
        canFocus,
        canCancel,
        canCounter,
        canRecall,
        canScuttle,
        pending,
      ].join(":");
      if (row.actionKey !== actionKey) {
        row.actions.replaceChildren();
        if (canFocus)
          row.actions.appendChild(
            operationAction(
              "focus_world",
              "home",
              SOW_t("hud.center_camera"),
              operation,
            ),
          );
        if (canCounter) {
          const counterOperation = Object.assign({}, operation, {
            incoming_troops: incomingTotals[playerId],
          });
          const counterButton = operationAction(
            "counter_attack",
            "ring_attack",
            SOW_t("hud.map_action_attack"),
            counterOperation,
          );
          counterButton.disabled = pending;
          row.actions.appendChild(counterButton);
        }
        if (canCancel) {
          const cancelLabel = SOW_t("endgame.cancel");
          row.actions.appendChild(
            operationAction("cancel_attack", "retreat", cancelLabel, operation),
          );
        }
        if (canRecall) {
          row.actions.appendChild(
            operationAction(
              "recall_fleet",
              "retreat",
              SOW_t("hud.recall_transport"),
              operation,
            ),
          );
        }
        if (canScuttle) {
          row.actions.appendChild(
            operationAction(
              "scuttle_fleet",
              "death",
              SOW_t("hud.destroy_transport"),
              operation,
            ),
          );
        }
        row.actionKey = actionKey;
      }
      row.actions
        .querySelectorAll<HTMLButtonElement>('[data-command="focus_world"]')
        .forEach(function (button) {
          button.dataset.focusX = String(operation.focus_x);
          button.dataset.focusY = String(operation.focus_y);
          const fleetId = operation.focus_fleet_id ??
            (operation.kind === "fleet" && outgoing ? operation.id : null);
          button.dataset.fleetId = fleetId == null ? "" : String(fleetId);
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

  function renderHoverCard(
    hov: NonNullable<MenuHudPayload["hovered"]> | null,
  ): void {
    if (!hudRefs!.hoverCard) return;
    if (hov) {
      hudRefs!.hoverCard.classList.remove("hidden");
      if (hudRefs!.hoverName)
        hudRefs!.hoverName.textContent = hov.name || SOW_t("hud.territory");
      if (hudRefs!.hoverPct)
        hudRefs!.hoverPct.textContent =
          Math.round((hov.territory_pct || 0) * 100) + "%";
      if (hudRefs!.hoverTroops)
        hudRefs!.hoverTroops.textContent =
          hov.troops > 1000
            ? (hov.troops / 1000).toFixed(1) + "k"
            : String(Math.floor(hov.troops || 0));
      if (hudRefs!.hoverGold)
        hudRefs!.hoverGold.textContent = Math.floor(
          hov.gold || 0,
        ).toLocaleString();
      if (hudRefs!.hoverBlds) {
        const bldText = [];
        if (hov.cities > 0)
          bldText.push(
            buildingIcon("City", 1, "sow-hud__inline-icon") + " x" + hov.cities,
          );
        if (hov.factories > 0)
          bldText.push(
            buildingIcon("Factory", 1, "sow-hud__inline-icon") +
              " x" +
              hov.factories,
          );
        if (hov.ports > 0)
          bldText.push(
            buildingIcon("Port", 1, "sow-hud__inline-icon") + " x" + hov.ports,
          );
        if (hov.towers > 0)
          bldText.push(
            buildingIcon("Tower", 1, "sow-hud__inline-icon") +
              " x" +
              hov.towers,
          );
        if (hov.farms > 0)
          bldText.push(
            buildingIcon("Farm", 1, "sow-hud__inline-icon") + " x" + hov.farms,
          );
        hudRefs!.hoverBlds.innerHTML = bldText.join(" ");
      }
    } else {
      hudRefs!.hoverCard.classList.add("hidden");
    }
  }

  function renderDiagnostics(
    fps: unknown,
    ping: unknown,
    visible: boolean,
  ): void {
    if (!hudRefs) return;
    if (hudRefs.diagnostics && hudRefs.diagnostics.hidden === visible) {
      hudRefs.diagnostics.hidden = !visible;
    }
    if (hudRefs.fps) {
      if (hudRefs.fps.hidden === visible) hudRefs.fps.hidden = !visible;
      if (visible) {
        const numericFps = typeof fps === "number" ? fps : Number(fps);
        const hasFps =
          fps !== null && Number.isFinite(numericFps) && numericFps > 0;
        const fpsText = SOW_t("hud.fps", { fps: hasFps ? numericFps : "--" });
        if (hudRefs.fps.textContent !== fpsText)
          hudRefs.fps.textContent = fpsText;
      }
    }
    if (hudRefs.ping) {
      const numericPing = typeof ping === "number" ? ping : Number(ping);
      const hasPing = visible && ping !== null && Number.isFinite(numericPing);
      if (hudRefs.ping.hidden === hasPing) hudRefs.ping.hidden = !hasPing;
      if (hasPing) {
        const pingText = Math.floor(numericPing) + "ms";
        if (hudRefs.ping.textContent !== pingText)
          hudRefs.ping.textContent = pingText;
      }
    }
  }

  function renderHud(forceNotifications = false): void {
    if (!hudRoot) return;
    if (!hudState || hudState.phase !== "Playing" || !hudState.hud) {
      closeLeaderboard();
      hudRoot.hidden = true;
      if (hudDevLayer) hudDevLayer.hidden = true;
      clearCompactHudDensity();
      activeHudPanel = null;
      betrayalOpen = false;
      emoteTextKey = "";
      devSidebarOpen = false;

      hudRoot.dataset.overlayOpen = "false";
      hudRoot.dataset.notificationsVisible = "false";
      hudRoot.dataset.requestToastsVisible = "false";
      if (hudRefs && hudRefs.mapMenu) hudRefs.mapMenu.classList.add("hidden");
      leaderboardRows = Object.create(null);
      leaderboardTeamRows = Object.create(null);
      leaderboardRenderKey = "";
      lastLeaderboardTeams = [];
      lastLeaderboardPlayersPayload = null;
      lastLeaderboardTeamsPayload = null;
      leaderboardOpenedAtMs = 0;
      leaderboardSyncRetryAtMs = 0;
      inboxFilter = "all";
      latestInboxRequests = [];
      inboxDataVersion = 0;
      pendingFreshInbox = false;
      requestToastStateVersion = 0;
      requestToastRenderKey = "";
      lastRequestToastsVisible = null;
      lastRequestToastsStackCount = -1;
      lastAllianceInboxCount = -1;
      requestCountdowns.clear();
      resetRequestToastDeck(requestToastDeck);
      clearLeavingRequestToasts();
      stopRequestTimers();
      if (hudRefs) {
        hudRefs.inboxRequests = Object.create(null);
        hudRefs.inboxRequestKeys = [];
        hudRefs.inboxRenderKey = "";
        if (hudRefs.inboxBulk) hudRefs.inboxBulk.hidden = true;
        if (hudRefs.inboxRows) hudRefs.inboxRows.replaceChildren();
        hudRefs.requestToastCards = [];
        if (hudRefs.requestToastStack)
          hudRefs.requestToastStack.replaceChildren();
        if (hudRefs.requestToasts) hudRefs.requestToasts.hidden = true;
        if (hudRefs.requestToastMore) hudRefs.requestToastMore.hidden = true;
      }
      if (hudRefs && hudRefs.rows) hudRefs.rows.replaceChildren();
      if (notificationTimer) window.clearTimeout(notificationTimer);
      notificationTimer = null;
      if (typeof bellRingTimer !== "undefined" && bellRingTimer) {
        if (typeof clearTimeout === "function") clearTimeout(bellRingTimer);
        bellRingTimer = null;
      }
      if (typeof lastInboxCount !== "undefined") {
        lastInboxCount = -1;
      }
      if (hudRefs && hudRefs.inboxButton && hudRefs.inboxButton.classList) {
        if (typeof hudRefs.inboxButton.classList.remove === "function") {
          hudRefs.inboxButton.classList.remove("is-ringing");
        } else if (typeof hudRefs.inboxButton.classList.toggle === "function") {
          hudRefs.inboxButton.classList.toggle("is-ringing", false);
        }
      }
      activeNotifications = [];
      renderCombatOperations([]);
      pendingCounterAttacks = Object.create(null);
      clearAllNotificationCards();
      notificationCursor = 0;
      mapFeedbackCursor = 0;
      clearMapFeedback();
      return;
    }
    const needsScreenMeasure = !hudInitialized || hudRoot.hidden;
    ensureHudDom();
    hudRoot.hidden = false;
    const hudScreen = needsScreenMeasure
      ? measureHudScreen()
      : cachedHudScreen();
    syncCompactHudDensity(hudScreen);

    const hud = hudState.hud;
    hudRefs!.awaitingPlayers.hidden = !hud.awaiting_players;
    const settings = hudState.settings || DEFAULT_HUD_SETTINGS;
    const settingsOpen = activeHudPanel === "settings";
    const inboxOpen = activeHudPanel === "inbox";
    const leaderboardOpen = activeHudPanel === "leaderboard";
    if (leaderboardOpen) leaderboardPanelOpen = true;
    else closeLeaderboard();
    hudRoot.classList.toggle(
      "is-reduced-motion",
      Boolean(hudState.settings && hudState.settings.reduced_motion),
    );
    const explicitSkin =
      hudState.selected_skin !== undefined
        ? typeof hudState.selected_skin === "string"
          ? hudState.selected_skin.trim()
          : ""
        : null;
    const activeSkin =
      explicitSkin !== null
        ? explicitSkin
        : (typeof document !== "undefined" &&
            document.documentElement &&
            document.documentElement.dataset.sowSkin) ||
          "";
    if (activeSkin) {
      if (hudRoot.dataset.skin !== activeSkin) hudRoot.dataset.skin = activeSkin;
      if (hudDevLayer && hudDevLayer.dataset.skin !== activeSkin)
        hudDevLayer.dataset.skin = activeSkin;
      if (
        typeof document !== "undefined" &&
        document.documentElement &&
        document.documentElement.dataset.sowSkin !== activeSkin
      ) {
        document.documentElement.dataset.sowSkin = activeSkin;
      }
    } else {
      if (hudRoot.dataset.skin) delete hudRoot.dataset.skin;
      if (hudDevLayer && hudDevLayer.dataset.skin) delete hudDevLayer.dataset.skin;
      if (
        explicitSkin !== null &&
        typeof document !== "undefined" &&
        document.documentElement &&
        document.documentElement.dataset.sowSkin
      ) {
        delete document.documentElement.dataset.sowSkin;
      }
    }
    const devTools = hud.dev_tools;
    const showDevTools =
      settings.show_dev_tools_available && settings.show_dev_tools;
    const diagnosticsVisible = settings.show_fps_ping === true;
    renderDiagnostics(hud.fps, hud.ping, diagnosticsVisible);
    if (hudRefs!.devBtn) {
      hudRefs!.devBtn.classList.toggle("hidden", !showDevTools);
      hudRefs!.devBtn.hidden = !showDevTools;
    }
    if (hudDevLayer) hudDevLayer.hidden = !showDevTools && !devSidebarOpen;
    if (showDevTools && !devOverridesRestored) {
      restoreDevOverridesIfNeeded({
        open: devTools.open === true,
        config: devTools.config,
        schema: devTools.schema || [],
        groups: devTools.groups || [],
        perf: devTools.perf,
        preview: devTools.preview,
      });
    }
    if (devSidebarOpen) {
      if (devTools.schema && hudRefs!.devSidebar) renderDevControls(devTools);
      const devConfig = devTools.config || {};
      if (hudRefs!.devSidebar) {
        hudRefs!.devSidebar
          .querySelectorAll<HTMLInputElement>("[data-dev]")
          .forEach(function (input) {
            if (document.activeElement === input) return;
            const field = input.dataset.dev;
            if (!field) return;
            const value = devConfig[field];
            if (value == null) return;
            const control = input.closest<HTMLElement>("[data-dev-control]");
            if (!control) return;
            const decimals = Number(control.dataset.devDecimals) || 0;
            if (control.dataset.devKind === "bool")
              input.checked = Boolean(value);
            else if (input.type === "radio")
              input.checked = Number(input.value) === Number(value);
            else if (
              control.dataset.devKind === "color" ||
              control.dataset.devKind === "vector"
            ) {
              if (
                input.hasAttribute("data-dev-channel") &&
                Array.isArray(value)
              ) {
                const part = value[Number(input.dataset.devChannel)];
                if (part === undefined) return;
                input.value =
                  input.type === "range"
                    ? String(part)
                    : devFormat(part, decimals);
                devReadout(input, decimals);
              }
            } else {
              const numericValue =
                typeof value === "number" ? value : Number(value);
              input.value =
                input.type === "range"
                  ? String(numericValue)
                  : devFormat(numericValue, decimals);
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
    }
    const gold = Math.floor(hud.gold || 0);
    const goldRate = Number(hud.gold_rate) || 0;
    const troops = Math.floor(hud.troops || 0);
    const maxTroops = Math.floor(hud.max_troops || 0);
    const pendingTroops = (hud.operations || []).reduce(function (
      total,
      operation,
    ) {
      if (operation.direction !== "outgoing") return total;
      const amount = Number(operation.troops);
      return Number.isFinite(amount) ? total + Math.max(0, amount) : total;
    }, 0);
    const prod = Math.floor(hud.troop_rate || 0);
    const currentRatio = Number.isFinite(hud.attack_ratio)
      ? hud.attack_ratio
      : DEFAULT_ATTACK_RATIO;
    const spawnSecs = hud.spawn_timer_secs;
    const isDeploying = spawnSecs != null && spawnSecs > 0;
    let hasBuildingItems = false;

    const goldText = formatHudMetric(gold);
    if (hudRefs!.plateGold && hudRefs!.plateGold.dataset.val !== goldText) {
      hudRefs!.plateGold.textContent = goldText;
      hudRefs!.plateGold.dataset.val = goldText;
    }
    const heroIncome = Number(hud.hero_income_rate) || 0;
    const goldRateText = formatHudRate(goldRate);
    if (hudRefs!.plateGoldRate) {
      hudRefs!.plateGoldRate.classList.toggle("is-hero-active", heroIncome > 0);
      const rateKey =
        heroIncome > 0
          ? `${hud.player_leader}:${heroIncome.toFixed(1)}`
          : "base";
      if (hudRefs!.plateGoldRate.dataset.sowTooltipKey !== rateKey) {
        hudRefs!.plateGoldRate.dataset.sowTooltipKey = rateKey;
        SOWTooltip.set(
          hudRefs!.plateGoldRate,
          heroIncome > 0
            ? {
                title: SOW_t("hud.gold_net_rate"),
                eyebrow: SOW_t(
                  "site.leader_" + hud.player_leader + "_ability",
                  {},
                ),
                text: "+" + formatHudRate(heroIncome),
                tone: "gold",
              }
            : { text: SOW_t("hud.gold_net_rate"), tone: "gold" },
        );
      }
    }
    if (
      hudRefs!.plateGoldRate &&
      hudRefs!.plateGoldRate.textContent !== goldRateText
    ) {
      hudRefs!.plateGoldRate.textContent = goldRateText;
    }
    if (hudRefs!.plateGoldRate)
      hudRefs!.plateGoldRate.classList.toggle("is-negative", goldRate < 0);
    if (hudRefs!.plateGoldWrap)
      hudRefs!.plateGoldWrap.classList.toggle("is-in-debt", gold < 0);
    const troopRateText = formatHudRate(prod);
    if (
      hudRefs!.plateTroopRate &&
      hudRefs!.plateTroopRate.textContent !== troopRateText
    ) {
      hudRefs!.plateTroopRate.textContent = troopRateText;
    }

    const playerPlateHidden = Boolean(hud.is_spectating);
    if (hudRefs!.nameplate) {
      if (hudRefs!.nameplate.hidden !== playerPlateHidden)
        hudRefs!.nameplate.hidden = playerPlateHidden;
      if (!playerPlateHidden) {
        const plateSlug = hud.player_art_slug || "null";
        const plateSrc = asset(
          avatarPortraitPath(plateSlug, hud.player_avatar_expression),
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
                plateSlug,
                hud.player_avatar_transition
                  ? hud.player_avatar_transition.to
                  : hud.player_avatar_expression,
              ),
            ),
          );
        if (
          hudRefs!.plateName &&
          hudRefs!.plateName.textContent !== platePlayerName
        ) {
          hudRefs!.plateName.textContent = platePlayerName;
        }
        const plateText =
          formatNameplateTroops(troops) +
          " / " +
          formatNameplateTroops(maxTroops);
        if (
          hudRefs!.plateTroops &&
          hudRefs!.plateTroops.textContent !== plateText
        ) {
          hudRefs!.plateTroops.textContent = plateText;
        }
        const plateFillPct =
          maxTroops > 0
            ? Math.min(100, Math.max(0, (troops / maxTroops) * 100))
            : 0;
        const plateFillStr = plateFillPct.toFixed(1) + "%";
        if (
          hudRefs!.plateFill &&
          hudRefs!.plateFill.style.width !== plateFillStr
        ) {
          hudRefs!.plateFill.style.width = plateFillStr;
        }
        const pendingFillPct =
          maxTroops > 0 ? Math.min(100, (pendingTroops / maxTroops) * 100) : 0;
        // ponytail: cap commitments at one bar width; add overflow UI if above-cap troops
        // need exact display.
        const pendingStartStr =
          Math.min(plateFillPct, 100 - pendingFillPct).toFixed(1) + "%";
        const pendingFillStr = pendingFillPct.toFixed(1) + "%";
        if (
          hudRefs!.platePendingTroops.style.insetInlineStart !== pendingStartStr
        ) {
          hudRefs!.platePendingTroops.style.insetInlineStart = pendingStartStr;
        }
        if (hudRefs!.platePendingTroops.style.width !== pendingFillStr) {
          hudRefs!.platePendingTroops.style.width = pendingFillStr;
        }
        hudRefs!.platePendingTroops.hidden = pendingFillPct <= 0;
        hudRefs!.plateFill.parentElement?.classList.toggle(
          "has-pending",
          pendingFillPct > 0,
        );
      }
    }

    if (hudRefs!.inboxCount) {
      const currentInboxCount = Number(hud.inbox_count || 0);
      if (lastInboxCount !== currentInboxCount) {
        hudRefs!.inboxCount.textContent = String(currentInboxCount);
        hudRefs!.inboxCount.hidden = !currentInboxCount;
        lastInboxCount = currentInboxCount;
      }
      const currentAllianceCount =
        hud.alliance_inbox_count != null
          ? Number(hud.alliance_inbox_count)
          : currentInboxCount;
      if (
        lastAllianceInboxCount >= 0 &&
        currentAllianceCount > lastAllianceInboxCount &&
        typeof triggerBellAnimation === "function"
      ) {
        triggerBellAnimation();
      }
      lastAllianceInboxCount = currentAllianceCount;
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
    hudRoot.dataset.deploying = String(isDeploying);
    if (hudRefs!.deployBtn) {
      const hasSpawned = Boolean(hud.has_spawned);
      const spawnDuration =
        Number.isFinite(hud.spawn_duration_secs) &&
        Number(hud.spawn_duration_secs) > 0
          ? Number(hud.spawn_duration_secs)
          : 15;
      const urgency =
        !isDeploying || spawnSecs > 5
          ? "normal"
          : spawnSecs <= 3
            ? "critical"
            : "warning";
      hudRefs!.deployBtn.hidden = !isDeploying;
      hudRefs!.deployBtn.classList.toggle("hidden", !isDeploying);
      hudRefs!.deployBtn.style.display = isDeploying ? "grid" : "none";
      hudRefs!.deployBtn.dataset.spawned = String(hasSpawned);
      hudRefs!.deployBtn.dataset.urgency = urgency;
      if (hasSpawned) {
        hudRefs!.deployBtn.dataset.command = "center_camera";
        hudRefs!.deployBtn.setAttribute("role", "button");
        hudRefs!.deployBtn.setAttribute("tabindex", "0");
        const centerLabel = SOW_t("hud.center_camera");
        hudRefs!.deployBtn.setAttribute("aria-label", centerLabel);
        SOWTooltip.set(hudRefs!.deployBtn, centerLabel);
      } else {
        delete hudRefs!.deployBtn.dataset.command;
        hudRefs!.deployBtn.removeAttribute("role");
        hudRefs!.deployBtn.removeAttribute("tabindex");
        hudRefs!.deployBtn.removeAttribute("aria-label");
        SOWTooltip.clear(hudRefs!.deployBtn);
      }
      if (hudRefs!.deployStatus) {
        hudRefs!.deployStatus.hidden = !hasSpawned;
      }
      if (isDeploying && hudRefs!.deployTimer) {
        const timerText = spawnSecs.toFixed(1) + "s";
        if (hudRefs!.deployTimer.textContent !== timerText) {
          hudRefs!.deployTimer.textContent = timerText;
        }
      }
      if (hudRefs!.deployFill) {
        const ratio = isDeploying
          ? Math.max(0, Math.min(1, spawnSecs / spawnDuration)).toFixed(4)
          : "0";
        if (hudRefs!.deployFill.dataset.barRatio !== ratio) {
          hudRefs!.deployFill.dataset.barRatio = ratio;
          hudRefs!.deployFill.style.setProperty("--sow-bar-ratio", ratio);
        }
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
        const reason = mapActionReason(
          { disabled: !affordable, cost: cost },
          gold,
        );
        button.setAttribute("aria-disabled", String(Boolean(reason)));
        button.classList.toggle("is-unaffordable", disabled);
        button.classList.toggle("active", selected);
        button.setAttribute("aria-pressed", String(selected));
        const costText = hasCost ? Math.floor(cost).toLocaleString() + "g" : "";
        const label =
          button.dataset.label || button.getAttribute("aria-label") || kind;
        button.dataset.label = label;
        const accessibleLabel = reason
          ? label + ". " + reason
          : costText
            ? label + " " + costText
            : label;
        if (button.dataset.sowTooltipKey !== accessibleLabel) {
          button.dataset.sowTooltipKey = accessibleLabel;
          button.setAttribute("aria-label", accessibleLabel);
          SOWTooltip.set(button, {
            title: label,
            text: reason || costText || undefined,
            tone: reason ? "danger" : "gold",
          });
        }
      });
      hasBuildingItems = hudRefs!.buildingButtons.some(function (button) {
        return !button.hidden;
      });
      hudRefs!.buildingsStrip.style.display =
        !isDeploying && hasBuildingItems ? "flex" : "none";
    }
    const nuclearStrip = document.getElementById("sow-hud-nuclear-strip");
    if (nuclearStrip) {
      const unlocked = campaignAllowsBuilding(campaignUnlocks(hud), "MissileSilo", 1);
      nuclearStrip.style.display = isDeploying || !unlocked ? "none" : "flex";
      nuclearStrip.querySelectorAll<HTMLButtonElement>("button").forEach(function (button) {
        const cost = Number(hud.nuclear_prices?.[Number(button.dataset.nuclearIndex)]);
        setTileCost(button, cost);
        const hasReadyNuke = Boolean(hud.fleet_panel?.nuke_available);
        const hasNuclearCity = Boolean(hud.fleet_panel?.nuke_city_ready);
        const affordable = Number.isFinite(cost) && gold >= cost;
        const reason = !hasNuclearCity
          ? SOW_t("hud.nuke_requires_city")
          : !hasReadyNuke
            ? SOW_t("hud.nuke_no_charges")
            : affordable
              ? ""
              : SOW_t("hud.not_enough_gold", {
                  cost: Math.ceil(cost).toLocaleString(),
                  available: Math.floor(gold).toLocaleString(),
                });
        const label = SOW_t("hud.nuclear_" + String(button.dataset.kind).toLowerCase());
        const costText = Number.isFinite(cost) && cost > 0
          ? Math.floor(cost).toLocaleString() + "g"
          : "";
        const accessibleLabel = reason
          ? label + ". " + reason
          : costText
            ? label + " " + costText
            : label;
        button.disabled = false;
        button.classList.toggle("is-unaffordable", Boolean(reason));
        button.classList.toggle("active", hud.selected_nuke === button.dataset.kind);
        button.setAttribute("aria-pressed", String(hud.selected_nuke === button.dataset.kind));
        button.setAttribute("aria-disabled", "false");
        if (button.dataset.sowTooltipKey !== accessibleLabel) {
          button.dataset.sowTooltipKey = accessibleLabel;
          button.setAttribute("aria-label", accessibleLabel);
          SOWTooltip.set(button, {
            title: label,
            text: reason || costText || undefined,
            tone: reason ? "danger" : "gold",
          });
        }
      });
    }
    const fleetControlEl = document.getElementById("sow-hud-fleet-control");
    if (fleetControlEl) {
      const fc = !isDeploying ? hud.fleet_control : null;
      fleetControlEl.style.display = fc && fc.selected_count > 0 ? "flex" : "none";
      if (fc && fc.selected_count > 0) {
        const countEl = fleetControlEl.querySelector<HTMLElement>("[data-fleet-count]");
        const countText = "×" + fc.selected_count;
        if (countEl && countEl.textContent !== countText) countEl.textContent = countText;
        const modeEl = fleetControlEl.querySelector<HTMLElement>("[data-fleet-mode]");
        const modeText = SOW_t("hud.fleet_mode_" + fc.mode);
        if (modeEl && modeEl.textContent !== modeText) modeEl.textContent = modeText;
        const moveBtn = fleetControlEl.querySelector<HTMLButtonElement>('[data-fleet-order="move"]');
        const patrolBtn = fleetControlEl.querySelector<HTMLButtonElement>('[data-fleet-order="patrol"]');
        if (moveBtn) moveBtn.classList.toggle("active", Boolean(fc.move_targeting));
        if (patrolBtn) patrolBtn.classList.toggle("active", !fc.move_targeting && fc.mode === "patrolling");
        const hintEl = fleetControlEl.querySelector<HTMLElement>("[data-fleet-hint]");
        if (hintEl) hintEl.classList.toggle("hidden", !fc.move_targeting);
      }
    }
    syncEmoteTexts(hud);
    if (hudRefs!.devSidebar) {
      hudRefs!.devSidebar.classList.toggle(
        "hidden",
        !showDevTools || !devSidebarOpen,
      );
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
        const sfxInput = hudRefs!.settings.querySelector<HTMLInputElement>(
          '[data-hud-setting="sfx_volume"]',
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
        if (muteInput && document.activeElement !== muteInput)
          muteInput.checked = !settings.mute_all;
        for (const [sliderInput, rawVal, valSelector] of [
          [musicInput, settings.music_volume, "#sow-hud-music-value"],
          [sfxInput, settings.sfx_volume, "#sow-hud-sfx-value"],
        ] as const) {
          if (sliderInput && document.activeElement !== sliderInput) {
            const vol = rawVal == null ? 0.8 : rawVal;
            sliderInput.value = String(vol);
            const pct = Math.round(Math.max(0, Math.min(1, Number(vol))) * 100);
            sliderInput.style.setProperty("--sow-slider-fill", pct + "%");
            sliderInput.style.setProperty("--sow-music-fill", pct + "%");
            const valEl =
              hudRefs!.settings.querySelector<HTMLElement>(valSelector);
            if (valEl) valEl.textContent = pct + "%";
          }
        }
        if (motionInput && document.activeElement !== motionInput)
          motionInput.checked = Boolean(settings.reduced_motion);
        if (freeZoomInput && document.activeElement !== freeZoomInput)
          freeZoomInput.checked = Boolean(settings.free_zoom_out);
        const clockInput = hudRefs!.settings.querySelector<HTMLInputElement>('[data-hud-setting="show_match_clock"]');
        if (clockInput) clockInput.checked = settings.show_match_clock === true;
        if (fpsPingInput && document.activeElement !== fpsPingInput)
          fpsPingInput.checked = settings.show_fps_ping === true;
        if (devToolsInput && document.activeElement !== devToolsInput)
          devToolsInput.checked = Boolean(settings.show_dev_tools);
      }
    }

    if (pendingFreshInbox && Array.isArray(hud.inbox)) {
      pendingFreshInbox = false;
      latestInboxRequests = hud.inbox;
      inboxDataVersion++;
      requestToastStateVersion++;
      syncRequestCountdowns(latestInboxRequests, requestCountdowns, Date.now());
    } else if (
      Number(hud.inbox_count || 0) === 0 &&
      latestInboxRequests.length > 0
    ) {
      latestInboxRequests = [];
      requestCountdowns.clear();
      inboxDataVersion++;
      requestToastStateVersion++;
    }

    if (hudRefs!.inbox) {
      const inboxWasHidden = hudRefs!.inbox.classList.contains("hidden");
      hudRefs!.inbox.classList.toggle("hidden", !inboxOpen);
      if (inboxOpen) {
        if (hudRefs!.inboxButton && hudRefs!.inboxButton.classList) {
          if (typeof hudRefs!.inboxButton.classList.remove === "function") {
            hudRefs!.inboxButton.classList.remove("is-ringing");
          } else if (
            typeof hudRefs!.inboxButton.classList.toggle === "function"
          ) {
            hudRefs!.inboxButton.classList.toggle("is-ringing", false);
          }
        }
        if (typeof bellRingTimer !== "undefined" && bellRingTimer) {
          if (typeof clearTimeout === "function") clearTimeout(bellRingTimer);
          bellRingTimer = null;
        }
        renderInbox(latestInboxRequests);
        if (inboxWasHidden && hudRefs!.inboxClose) hudRefs!.inboxClose.focus();
      } else if (
        !inboxWasHidden &&
        hudRefs!.inbox.contains(document.activeElement) &&
        hudRefs!.inboxButton
      ) {
        hudRefs!.inboxButton.focus();
      }
    }
    renderRequestToasts(latestInboxRequests, inboxOpen);
    if (hudRefs!.transfer) {
      const transferWasHidden = hudRefs!.transfer.classList.contains("hidden");
      hudRefs!.transfer.classList.toggle("hidden", !hud.transfer);
      if (hud.transfer) {
        hudRefs!.transfer.dataset.targetId = String(hud.transfer.target_id);
        // The self-click panel speaks to every ally, so it names them all and hides one ally's stock.
        const allAllies = Number(hud.transfer.all_allies) || 0;
        const targetName =
          allAllies > 0
            ? SOW_t("hud.transfer_all_allies", { count: String(allAllies) })
            : hud.transfer.target_name;
        hudRefs!.transfer.dataset.allAllies = String(allAllies > 0);
        renderPlayerIdentity(
          hudRefs!.transferIdentity,
          targetName,
          hud.transfer.target_avatar,
        );
        hudRefs!.transferTarget.setAttribute(
          "aria-label",
          SOW_t("hud.target_player", {
            name: targetName || SOW_t("hud.player_name"),
          }),
        );
        const transferLive = hud.transfer;
        hudRefs!.transferStockTroops.textContent = formatNameplateTroops(
          transferLive.target_troops,
        );
        hudRefs!.transferStockMaxTroops.textContent = formatNameplateTroops(
          transferLive.target_max_troops,
        );
        hudRefs!.transferStockGold.textContent = formatNameplateTroops(
          transferLive.target_gold,
        );
        const modeKey =
          String(transferLive.mode || "") + ":" + String(allAllies > 0);
        const reseed =
          transferWasHidden ||
          hudRefs!.transfer.dataset.seededTarget !==
            String(transferLive.target_id) ||
          hudRefs!.transfer.dataset.seededMode !== modeKey;
        if (reseed) {
          delete transferFlips.troops;
          delete transferFlips.gold;
        }
        (["troops", "gold"] as const).forEach(function (resource) {
          const toggle =
            resource === "troops"
              ? hudRefs!.transferDirTroops
              : hudRefs!.transferDirGold;
          const asks =
            transferRowSide(resource, transferLive, hud) === "request";
          toggle.textContent = SOW_t(
            asks ? "hud.transfer_want" : "hud.transfer_give",
          );
          toggle.classList.toggle("is-want", asks);
        });
        const kind = transferKind(transferLive, hud);
        hudRefs!.transfer.dataset.transferKind = kind;
        hudRefs!.transferTitle.textContent =
          allAllies > 0 && kind === "request"
            ? SOW_t("hud.map_action_request_allies")
            : kind === "send"
              ? tWithFallback(
                  "hud.send_resources_title",
                  undefined,
                  SOW_t("hud.send"),
                )
              : kind === "request"
                ? tWithFallback(
                    "hud.request_resources_title",
                    undefined,
                    SOW_t("hud.request"),
                  )
                : tWithFallback(
                    "hud.trade_offer_title",
                    undefined,
                    SOW_t("hud.trade"),
                  );
        // The panel carries one action, the one the rows add up to; the others are hidden, not greyed out.
        const ctas = [
          [hudRefs!.transferSend, "send"],
          [hudRefs!.transferRequest, "request"],
          [hudRefs!.transferTrade, "trade"],
        ] as const;
        ctas.forEach(function (entry) {
          entry[0].hidden = entry[1] !== kind;
          entry[0].disabled =
            transferBlockReason(entry[1], transferLive, hud) !== null;
          entry[0].classList.toggle("is-primary", entry[1] === kind);
        });
        // Only the CTA in focus can explain a block; a hidden one's reason must not show.
        const hint = transferBlockReason(kind, transferLive, hud);
        hudRefs!.transferHint.textContent = hint ?? "";
        hudRefs!.transferHint.hidden = hint === null;
        if (reseed) {
          hudRefs!.transfer.dataset.seededTarget = String(
            transferLive.target_id,
          );
          hudRefs!.transfer.dataset.seededMode = modeKey;
          hudRefs!.transferGold.value = "0";
          hudRefs!.transferTroops.value = "0";
        }
        if (transferWasHidden) {
          const activeElement = document.activeElement;
          hudRefs!.transferReturnFocus =
            activeElement instanceof HTMLElement ? activeElement : null;
          hudRefs!.transferClose.focus({ preventScroll: true });
        }
      } else if (
        !transferWasHidden &&
        hudRefs!.transfer.contains(document.activeElement)
      ) {
        restoreHudFocus(hudRefs!.transferReturnFocus);
      }
    }
    if (hudRefs!.betrayal) {
      const betrayalWasHidden = hudRefs!.betrayal.classList.contains("hidden");
      const betrayal = hud.betrayal;
      betrayalOpen = Boolean(betrayal);
      hudRefs!.betrayal.classList.toggle("hidden", !betrayalOpen);
      if (betrayal) {
        hudRefs!.betrayalTitle.textContent = SOW_t(
          betrayal.is_teammate ? "hud.betray_teammate" : "hud.break_alliance",
        );
        hudRefs!.betrayalCopy.textContent = SOW_t(
          betrayal.is_teammate
            ? "hud.betray_teammate_body"
            : "hud.break_alliance_body",
        );
        hudRefs!.betrayalCancel.textContent = SOW_t(
          betrayal.is_teammate ? "endgame.cancel" : "hud.keep_alliance",
        );
        hudRefs!.betrayalConfirmLabel.textContent = SOW_t(
          betrayal.is_fleet ? "hud.map_action_fleet" : "hud.attack",
        );
        renderPlayerIdentity(
          hudRefs!.betrayalIdentity,
          betrayal.ally_name,
          betrayal.ally_avatar,
        );
        if (betrayalWasHidden) {
          const activeElement = document.activeElement;
          hudRefs!.betrayalReturnFocus =
            activeElement instanceof HTMLElement ? activeElement : null;
          hudRefs!.betrayalCancel.focus({ preventScroll: true });
        }
      } else if (
        !betrayalWasHidden &&
        hudRefs!.betrayal.contains(document.activeElement)
      ) {
        restoreHudFocus(hudRefs!.betrayalReturnFocus);
      }
    }

    renderNotifications(hud.notifications, forceNotifications, hudScreen);
    renderCombatOperations(hud.operations);
    renderMapFeedback(hud.map_feedback);

    const territoryScore = document.getElementById("sow-hud-territory-score")!;
    const teamScores = hud.team_leaderboard;
    territoryScore.hidden = !Array.isArray(teamScores);
    if (Array.isArray(teamScores)) {
      territoryScore.style.setProperty("--territory-goal", String((hud.territory_goal ?? 0) * 100) + "%");
      for (const child of territoryScore.children) {
        const side = child as HTMLElement;
        const team = side.dataset.side;
        const share = Number(teamScores.find((entry: { team: string }) => entry.team === team)?.territory_pct ?? 0);
        const own = hud.local_team === team;
        const percent = Math.round(share * 100) + "%";
        if (side.firstElementChild!.textContent !== percent) side.firstElementChild!.textContent = percent;
        side.classList.toggle("is-own", own);
        side.style.setProperty("--territory-share", String(share * 100) + "%");
        const label = hud.local_team ? (own ? "own" : "rival") : team === "Red" ? "first" : "second";
        side.setAttribute("aria-label", SOW_t("hud.territory_" + label, { percent }));
      }
    }
    const matchClock = document.getElementById("sow-hud-match-clock")!;
    const seconds = hud.match_clock_seconds;
    matchClock.hidden = typeof seconds !== "number";
    if (typeof seconds === "number") {
      const time = String(Math.floor(seconds / 60)).padStart(2, "0") + ":" + String(seconds % 60).padStart(2, "0");
      if (matchClock.textContent !== time) matchClock.textContent = time;
    }

    // Leaderboard
    if (hudRefs!.leaderboard) {
      hudRefs!.leaderboard.classList.toggle("hidden", !leaderboardOpen);
    }
    if (leaderboardOpen) {
      if (
        hudRefs!.lbSearch &&
        hudRefs!.lbSearch.value !== leaderboardFilterQuery
      ) {
        hudRefs!.lbSearch.value = leaderboardFilterQuery;
      }
      if (hud.leaderboard_open === false) {
        const now = Date.now();
        if (
          now - leaderboardOpenedAtMs >= 350 &&
          now - leaderboardSyncRetryAtMs >= 1000
        ) {
          leaderboardSyncRetryAtMs = now;
          send("toggle_leaderboard", { open: true });
        }
      } else if (hud.leaderboard_open === true) {
        leaderboardSyncRetryAtMs = 0;
      }
      if (Array.isArray(hud.leaderboard)) {
        const teams = Array.isArray(hud.team_leaderboard)
          ? hud.team_leaderboard
          : lastLeaderboardTeams;
        if (
          hud.leaderboard !== lastLeaderboardPlayersPayload ||
          teams !== lastLeaderboardTeamsPayload
        ) {
          lastLeaderboardPlayersPayload = hud.leaderboard;
          lastLeaderboardTeamsPayload = teams;
          updateLeaderboard(hud.leaderboard, teams);
        }
      }
    }

    // Surrender Modal
    if (hudRefs!.surrender) {
      hudRefs!.surrender.classList.toggle("hidden", !surrenderModalOpen);
      if (surrenderModalOpen) {
        const tutorialActive = Boolean(hud.tutorial && hud.tutorial.active);
        const surrenderLeader = leaderById(
          (hud && hud.player_leader) || (hudState && hudState.selected_leader),
        );
        if (hudRefs!.surrenderTitle)
          hudRefs!.surrenderTitle.textContent = tutorialActive
            ? SOW_t("endgame.leave_tutorial")
            : SOW_t("endgame.leave");
        hudRefs!.surrenderStats?.classList.toggle("hidden", tutorialActive);
        if (!tutorialActive)
          renderEndgameStats(
            hudRefs!.surrenderStatValues,
            hud.player_endgame_stats,
          );
        if (hudRefs!.surrenderPortrait) {
          setAvatarImage(
            hudRefs!.surrenderPortrait,
            asset(
              "gameplay/avatars/" + (hud.player_art_slug || "null") + ".webp",
            ),
          );
          hudRefs!.surrenderPortrait.alt =
            surrenderLeader.name || SOW_t("hud.leader_avatar");
        }
        if (hudRefs!.surrenderCancel)
          hudRefs!.surrenderCancel.textContent = SOW_t("endgame.cancel");
        if (hudRefs!.surrenderActionLabel)
          hudRefs!.surrenderActionLabel.textContent = tutorialActive
            ? SOW_t("endgame.leave_tutorial")
            : SOW_t("endgame.leave");
      }
    }

    // Endgame Screen
    const isOver = Boolean(hud.match_over);
    const isWinner = Boolean(hud.is_winner);
    if (hudRefs!.endgame) {
      hudRefs!.endgame.classList.toggle("hidden", !isOver);
      const endgameJustOpened = isOver && !wasEndgameOpen;
      if (endgameJustOpened) {
        hudRefs!.endgame.classList.remove("is-entering");
        if (typeof window.requestAnimationFrame === "function") {
          const modalEl = hudRefs!.endgame;
          window.requestAnimationFrame(function () {
            if (!modalEl.classList.contains("hidden")) modalEl.classList.add("is-entering");
          });
        } else {
          hudRefs!.endgame.classList.add("is-entering");
        }
      } else if (!isOver && wasEndgameOpen) {
        hudRefs!.endgame.classList.remove("is-entering");
        endgameRollupCancel?.();
        endgameRollupCancel = null;
      }
      wasEndgameOpen = isOver;
      if (isOver) {
        const result = isWinner ? "victory" : "defeat";
        const stats = hud.player_endgame_stats || {};
        if (hudRefs!.endgameCard) hudRefs!.endgameCard.dataset.result = result;
        if (hudRefs!.endgameBanner)
          hudRefs!.endgameBanner.textContent = isWinner
            ? SOW_t("hud.victory")
            : SOW_t("hud.defeat");
        const activeLeaderId =
          (hud && hud.player_leader) || (hudState && hudState.selected_leader);
        const activeLeader = leaderById(activeLeaderId);
        if (hudRefs!.endgamePortrait) {
          setAvatarImage(
            hudRefs!.endgamePortrait,
            asset(
              "gameplay/avatars/" + (hud.player_art_slug || "null") + ".webp",
            ),
          );
          hudRefs!.endgamePortrait.alt =
            activeLeader.name || SOW_t("hud.leader_avatar");
        }
        const reason = document.getElementById("sow-hud-endgame-reason")!;
        reason.hidden = typeof hud.endgame_reason !== "string";
        if (!reason.hidden) {
          const winner = hud.winning_team
            ? SOW_t(
                hud.winning_team === "Red"
                  ? "hud.filter_red_team"
                  : "hud.filter_blue_team",
              )
            : hud.winner_name || SOW_t("hud.victory");
          reason.textContent = SOW_t("endgame.reason_" + hud.endgame_reason, {
            winner,
            percent: 75,
          });
        }
        if (endgameJustOpened && !Boolean(hudState?.settings?.reduced_motion)) {
          startEndgameStatRollup(hudRefs!.endgameStatValues, stats);
        } else if (!endgameRollupActive) {
          renderEndgameStats(hudRefs!.endgameStatValues, stats);
        }
        if (hudRefs!.endgameObserve)
          hudRefs!.endgameObserve.classList.toggle(
            "hidden",
            isWinner || Boolean(hud.winner_name) || hud.endgame_reason === "no_winner",
          );
      }
    }
    const mapMenuOpen = renderMapMenu(hud.map_menu);
    renderPlacementPanel(hud.building_placement);
    const openBuilding = mapMenuOpen && hud.map_menu?.view === "building_details"
      ? hud.map_menu.building
      : null;
    renderWorldBuildingProgress(
      hud.buildings_in_progress,
      openBuilding?.production?.warship ? hud.map_menu?.tile_idx : undefined,
    );
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
      if (button.getAttribute("aria-expanded") !== value)
        button.setAttribute("aria-expanded", value);
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
    const dockHidden = !(isDeploying || hasBuildingItems);
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
    document.addEventListener("pointerdown", function (event) {
      if (!hudRefs) return;
      const target = event.target;
      if (!(target instanceof Element)) return;
      if (
        hudRefs.settings &&
        !hudRefs.settings.classList.contains("hidden") &&
        !hudRefs.settings.contains(target) &&
        !target.closest('[data-command="toggle_settings"]') &&
        !target.closest(".sow-control-dropdown__menu")
      ) {
        setHudPanel(null);
        renderHud();
      }
      if (
        activeHudPanel === "inbox" &&
        hudRefs.inbox &&
        !hudRefs.inbox.classList.contains("hidden") &&
        !hudRefs.inbox.contains(target) &&
        !target.closest('[data-command="toggle_inbox"]')
      ) {
        setHudPanel(null);
        renderHud();
      }
      if (
        hudRefs.requestToasts &&
        !hudRefs.requestToasts.hidden &&
        !hudRefs.requestToasts.contains(target) &&
        !target.closest('[data-command="toggle_inbox"]')
      ) {
        for (const card of hudRefs.requestToastCards) {
          pendingToastExitModes.set(card.requestKey, "dismiss");
        }
        if (dismissAllRequestToasts(requestToastDeck)) {
          requestToastStateVersion++;
          renderRequestToasts(latestInboxRequests, activeHudPanel === "inbox");
        }
      }
    });
    hudRoot.addEventListener("pointerdown", function (event) {
      const target = event.target;
      if (!(target instanceof Element)) return;
      const button = target.closest(
        '[data-command="zoom_in"], [data-command="zoom_out"]',
      );
      if (
        !(button instanceof HTMLButtonElement) ||
        button.disabled ||
        event.button !== 0
      )
        return;
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
        if (
          !emoteKind ||
          (emoteChoice as HTMLButtonElement).disabled ||
          emoteChoice.getAttribute("aria-disabled") === "true"
        ) {
          return;
        }
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
        send("set_emote_pinned", {
          pinned: pinButton.getAttribute("aria-pressed") !== "true",
        });
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
        document.getElementById("blade")?.focus({ preventScroll: true });
        return;
      }
      const productionBtn = target.closest<HTMLButtonElement>("[data-production-kind]");
      if (productionBtn) {
        event.preventDefault();
        event.stopPropagation();
        const kind = productionBtn.dataset.productionKind;
        const buildingId = Number(productionBtn.dataset.buildingId);
        if (kind && Number.isSafeInteger(buildingId) && buildingId > 0) {
          send("building_production", { building_id: buildingId, kind });
        }
        return;
      }
      const productionMaxBtn = target.closest<HTMLButtonElement>("[data-production-max]");
      if (productionMaxBtn) {
        event.preventDefault();
        event.stopPropagation();
        const kind = productionMaxBtn.dataset.productionMax;
        const buildingId = Number(productionMaxBtn.dataset.buildingId);
        if (kind && Number.isSafeInteger(buildingId) && buildingId > 0) {
          send("building_production_max", { building_id: buildingId, kind });
        }
        return;
      }
      const cancelQueueBtn = target.closest<HTMLButtonElement>("[data-cancel-warship-index]");
      if (cancelQueueBtn) {
        event.preventDefault();
        event.stopPropagation();
        const index = Number(cancelQueueBtn.dataset.cancelWarshipIndex);
        const buildingId = Number(cancelQueueBtn.dataset.buildingId);
        if (Number.isSafeInteger(index) && index >= 0 && Number.isSafeInteger(buildingId) && buildingId > 0) {
          send("cancel_warship_queue", { building_id: buildingId, index });
        }
        return;
      }
      const fleetOrderBtn = target.closest<HTMLButtonElement>("[data-fleet-order]");
      if (fleetOrderBtn) {
        event.preventDefault();
        event.stopPropagation();
        const order = fleetOrderBtn.dataset.fleetOrder;
        if (order === "move") {
          send("warship_order_move");
        } else if (order === "patrol") {
          send("warship_order_patrol");
        } else if (order === "clear") {
          send("warship_clear_selection");
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
          mapMenuView =
            mapGroup.dataset.mapGroup === "emote" ? "emote" : "build";
          renderMapMenu(hudState && hudState.hud && hudState.hud.map_menu);
          const firstChoice = mapMenu.querySelector<HTMLButtonElement>(
            ".sow-hud__ring-slot:not(:disabled)",
          );
          if (firstChoice) firstChoice.focus({ preventScroll: true });
        } else if (mapButton) {
          if (mapButton.disabled) return;
          // Hold-to-confirm actions only fire from a completed hold.
          if (mapButton.dataset.ringHold === "true" && !ring.holdConfirmed)
            return;
          const action = mapButton.dataset.mapAction;
          if (!action) return;
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
      if (!btn) {
        if (target.closest(".sow-hud__request-toast")) {
          const front = requestToastDeck.entries[0];
          if (front) pendingToastExitModes.set(front.key, "dismiss");
          if (dismissFrontRequestToast(requestToastDeck)) {
            requestToastStateVersion++;
            renderRequestToasts(
              latestInboxRequests,
              activeHudPanel === "inbox",
            );
          }
        }
        return;
      }
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
      } else if (cmd === "dev_save_settings") {
        downloadDevSettings();
      } else if (cmd === "dev_copy_settings") {
        copyDevSettings(btn);
      } else if (cmd === "reset_dev_config") {
        removeDevOverride({
          section: btn.dataset.devSection || null,
          field: btn.dataset.devField || null,
        });
        send("reset_dev_config", {
          section: btn.dataset.devSection || null,
          field: btn.dataset.devField || null,
        });
      } else if (cmd === "dev_step") {
        if (event.detail === 0 && btn instanceof HTMLButtonElement)
          devStepOnce(btn);
      } else if (cmd === "dev_tab") {
        devTab = btn.dataset.devTab || "";
        try {
          window.localStorage.setItem("sow_dev_tab", devTab);
        } catch {
          /* storage unavailable */
        }
        const devSearch =
          hudRefs!.devSidebar!.querySelector<HTMLInputElement>(
            "[data-dev-search]",
          );
        if (devSearch) devSearch.value = "";
        renderDevControls(
          (hudState && hudState.hud && hudState.hud.dev_tools) || {},
        );
        const devBody =
          hudRefs!.devSidebar.querySelector<HTMLElement>(".sow-hud__dev-body");
        if (devBody) devBody.scrollTop = 0;
      } else if (cmd === "dev_menu") {
        const devMenu =
          hudRefs!.devSidebar!.querySelector<HTMLElement>("#sow-hud-dev-menu");
        if (devMenu) setDevMenu(devMenu.hasAttribute("hidden"));
      } else if (cmd === "dev_preview") {
        send("set_dev_vfx_preview", { target: Number(btn.dataset.devTarget) });
        send("replay_dev_vfx_preview");
      } else if (cmd === "dev_reset_all") {
        if (devResetTimer) {
          setDevMenu(false);
          removeDevOverride({});
          send("reset_dev_config", { section: null, field: null });
        } else {
          btn.textContent = "Tap again to reset all";
          devResetTimer = window.setTimeout(disarmDevReset, 3000);
        }
      } else if (cmd === "dev_perf") {
        const group = btn.dataset.devPerf;
        if (group)
          send("dev_vfx_group", { group, on: btn.dataset.state === "off" });
      } else if (cmd === "dev_vfx_all") {
        setDevMenu(false);
        send("dev_vfx_all", { on: btn.dataset.on === "1" });
      } else if (cmd === "clear_emote") {
        send("clear_emote");
      } else if (cmd === "open_transfer") {
        setHudPanel(null);
        send("open_transfer", {
          target_player_id: Number(btn.dataset.playerId),
        });
        renderHud();
      } else if (cmd === "close_transfer") {
        send("close_transfer");
      } else if (cmd === "set_transfer_amount") {
        const live = hudState && hudState.hud;
        const offer = live && live.transfer;
        const troops = btn.dataset.resource === "troops";
        const input =
          hudRefs && (troops ? hudRefs.transferTroops : hudRefs.transferGold);
        if (live && offer && input)
          input.value = String(
            transferPresetAmount(
              troops ? "troops" : "gold",
              btn.dataset.percent || "0",
              offer,
              live,
            ),
          );
      } else if (cmd === "flip_transfer_row") {
        const live = hudState && hudState.hud;
        const offer = live && live.transfer;
        const resource = btn.dataset.resource === "gold" ? "gold" : "troops";
        if (live && offer && hudRefs) {
          transferFlips[resource] = !transferFlips[resource];
          // The row now sizes from the other stock, so it restarts at a quarter of it.
          (resource === "gold"
            ? hudRefs.transferGold
            : hudRefs.transferTroops
          ).value = String(transferPresetAmount(resource, "25", offer, live));
          renderHud();
        }
      } else if (
        cmd === "send_resources" ||
        cmd === "request_resources" ||
        cmd === "offer_trade"
      ) {
        const kind =
          cmd === "send_resources"
            ? "send"
            : cmd === "request_resources"
              ? "request"
              : "trade";
        const live = hudState && hudState.hud;
        const offer = live && live.transfer;
        const targetId =
          hudRefs && hudRefs.transfer
            ? Number(hudRefs.transfer.dataset.targetId)
            : 0;
        // Poka-yoke: each row sends only what its side can move, so nothing is clipped silently.
        if (
          live &&
          offer &&
          hudRefs &&
          targetId > 0 &&
          transferBlockReason(kind, offer, live) === null
        ) {
          const give = { gold: 0, troops: 0 };
          const ask = { gold: 0, troops: 0 };
          (["troops", "gold"] as const).forEach(function (resource) {
            const side =
              kind === "trade" ? transferRowSide(resource, offer, live) : kind;
            const input =
              resource === "gold"
                ? hudRefs!.transferGold
                : hudRefs!.transferTroops;
            const amount = Math.min(
              wholeAmount(Number(input.value)),
              transferCaps(side, offer, live)[resource],
            );
            (side === "send" ? give : ask)[resource] = amount;
          });
          const gives = give.gold > 0 || give.troops > 0;
          const asks = ask.gold > 0 || ask.troops > 0;
          if (asks) {
            transferRequestReadyAt[targetId] =
              Date.now() + TRANSFER_REQUEST_COOLDOWN_MS;
            // The wasm pushes state only when it changes, so re-render when the lock lifts.
            window.setTimeout(renderHud, TRANSFER_REQUEST_COOLDOWN_MS);
          }
          // A row left at zero turns a trade back into a plain Send or Request.
          if (offer.all_allies) {
            if (asks)
              send("offer_to_allies", {
                gold: ask.gold,
                troops: ask.troops,
                pay_gold: give.gold,
                pay_troops: give.troops,
              });
          } else if (gives && asks)
            send("offer_trade", {
              target_player_id: targetId,
              gold: ask.gold,
              troops: ask.troops,
              pay_gold: give.gold,
              pay_troops: give.troops,
            });
          else if (gives)
            send("send_resources", {
              target_player_id: targetId,
              gold: give.gold,
              troops: give.troops,
            });
          else if (asks)
            send("request_resources", {
              target_player_id: targetId,
              gold: ask.gold,
              troops: ask.troops,
            });
          if (gives || asks) renderHud();
        }
      } else if (
        cmd === "accept_alliance" ||
        cmd === "reject_alliance" ||
        cmd === "accept_resource_request" ||
        cmd === "reject_resource_request" ||
        cmd === "accept_help_call" ||
        cmd === "reject_help_call"
      ) {
        const requesterId = Number(btn.dataset.playerId);
        const cardEl = btn.closest<HTMLElement>("[data-request-key]");
        const key = cardEl ? cardEl.dataset.requestKey : undefined;
        const fromToast = Boolean(btn.closest(".sow-hud__request-toast"));
        if (requesterId > 0) send(cmd, { target_player_id: requesterId });
        if (key) {
          pendingToastExitModes.set(
            key,
            cmd.indexOf("accept_") === 0 ? "accept" : "dismiss",
          );
          if (dismissRequestToastByKey(requestToastDeck, key)) {
            requestToastStateVersion++;
          }
        }
        if (fromToast) {
          renderRequestToasts(latestInboxRequests, activeHudPanel === "inbox");
        } else if (
          activeHudPanel === "inbox" &&
          latestInboxRequests.filter(isValidInboxRequest).length <= 1
        ) {
          setHudPanel(null);
          renderHud();
        }
      } else if (cmd === "filter_inbox") {
        const nextFilter = btn.dataset.inboxFilter as
          | InboxCategoryFilter
          | undefined;
        if (
          nextFilter === "all" ||
          nextFilter === "alliance" ||
          nextFilter === "aid"
        ) {
          inboxFilter = nextFilter;
          renderInbox(latestInboxRequests);
        }
      } else if (
        cmd === "accept_all_inbox" ||
        cmd === "reject_all_inbox" ||
        cmd === "accept_all_alliances" ||
        cmd === "reject_all_alliances"
      ) {
        const isAcceptAll =
          cmd === "accept_all_inbox" || cmd === "accept_all_alliances";
        const bulkFilter: InboxCategoryFilter =
          cmd === "accept_all_alliances" || cmd === "reject_all_alliances"
            ? "alliance"
            : inboxFilter;
        const exitMode: RequestToastExitMode = isAcceptAll
          ? "accept"
          : "dismiss";
        let hasRemainingOutsideFilter = false;
        const seenBulkKeys = new Set<string>();
        latestInboxRequests.forEach(function (request) {
          if (!isValidInboxRequest(request)) return;
          const key = requestKey(request);
          if (seenBulkKeys.has(key)) return;
          seenBulkKeys.add(key);
          if (!matchesInboxFilter(request, bulkFilter)) {
            hasRemainingOutsideFilter = true;
            return;
          }
          const rendered = hudRefs && hudRefs.inboxRequests[key];
          if (
            isAcceptAll &&
            (!request.active || (rendered && rendered.accept.disabled))
          ) {
            return;
          }
          send(inboxRequestIntent(request, isAcceptAll), {
            target_player_id: request.requester_id,
          });
          pendingToastExitModes.set(key, exitMode);
          if (dismissRequestToastByKey(requestToastDeck, key)) {
            requestToastStateVersion++;
          }
        });
        if (!hasRemainingOutsideFilter && activeHudPanel === "inbox") {
          setHudPanel(null);
          renderHud();
        } else {
          renderRequestToasts(latestInboxRequests, activeHudPanel === "inbox");
        }
      } else if (cmd === "open_inbox_from_toast") {
        clearLeavingRequestToasts();
        if (dismissAllRequestToasts(requestToastDeck)) {
          requestToastStateVersion++;
        }
        setHudPanel("inbox");
        renderHud();
      } else if (cmd === "cancel_betrayal") {
        betrayalOpen = false;
        pendingCounterAttacks = Object.create(null);
        send("cancel_betrayal");
        renderHud();
      } else if (cmd === "confirm_betrayal") {
        betrayalOpen = false;
        send("confirm_betrayal");
        renderHud();
      } else if (
        cmd === "cancel_attack" ||
        cmd === "recall_fleet" ||
        cmd === "scuttle_fleet"
      ) {
        const isAttack = cmd === "cancel_attack";
        const id = Number(
          isAttack ? btn.dataset.attackId : btn.dataset.fleetId,
        );
        if (id > 0 && btn instanceof HTMLButtonElement)
          btn.disabled = send(
            cmd,
            isAttack ? { attack_id: id } : { fleet_id: id },
          );
      } else if (cmd === "counter_attack") {
        const counterTarget = Number(btn.dataset.playerId);
        const incomingTotal = Number(btn.dataset.incomingTroops);
        if (
          counterTarget > 0 &&
          incomingTotal > 0 &&
          pendingCounterAttacks[counterTarget] == null
        ) {
          pendingCounterAttacks[counterTarget] = incomingTotal;
          if (hudRefs && hudRefs.operationRows) {
            hudRefs.operationRows
              .querySelectorAll<HTMLButtonElement>(
                '[data-command="counter_attack"]',
              )
              .forEach(function (button) {
                if (Number(button.dataset.playerId) === counterTarget)
                  button.disabled = true;
              });
          }
          if (!send("counter_attack", { target_player_id: counterTarget })) {
            delete pendingCounterAttacks[counterTarget];
            if (hudRefs && hudRefs.operationRows) {
              hudRefs.operationRows
                .querySelectorAll<HTMLButtonElement>(
                  '[data-command="counter_attack"]',
                )
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
        if (Number.isFinite(focusX) && Number.isFinite(focusY)) {
          const fleetId = Number(btn.dataset.fleetId);
          send("focus_world", {
            x: focusX,
            y: focusY,
            ...(btn.dataset.requireOwnedFleet === "true"
              ? { require_owned_fleet: true }
              : {}),
            ...(Number.isSafeInteger(fleetId) && fleetId > 0
              ? { fleet_id: fleetId }
              : {}),
          });
        }
      } else if (cmd === "prompt_surrender") {
        if (hudState?.hud?.is_spectating) {
          send("return_to_menu");
          return;
        }
        send("request_endgame_stats");
        surrenderModalOpen = true;
        renderHud();
      } else if (cmd === "close_surrender_modal") {
        send("clear_endgame_stats");
        surrenderModalOpen = false;
        renderHud();
      } else if (cmd === "confirm_surrender") {
        surrenderModalOpen = false;
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
        if (kind) {
          send("select_building", { kind });
          document.getElementById("blade")?.focus({ preventScroll: true });
        }
      } else if (cmd === "select_nuke") {
        send("select_nuke", { kind: btn.dataset.kind || "AtomBomb" });
      } else if (cmd === "confirm_endgame_leave") {
        send("return_to_menu");
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
    const previousHud =
      hudState && hudState.phase === "Playing" ? hudState.hud : null;
    hudState = parsed;
    if (hudState.phase === "Playing") {
      if (!previousHud) emoteTextKey = "";
      hudState.hud = hudState.hud || previousHud;
      if (hudState.hud && Array.isArray(hudState.hud.inbox)) {
        pendingFreshInbox = true;
      } else if (
        hudState.hud &&
        previousHud &&
        Array.isArray(previousHud.inbox) &&
        (Number(hudState.hud.inbox_count || 0) > 0 || activeHudPanel === "inbox")
      ) {
        hudState.hud.inbox = previousHud.inbox;
      }
      // Hover arrives on its own channel (SOW_hover_card_update), so keep the last one.
      if (hudState.hud && previousHud)
        hudState.hud.hovered = previousHud.hovered;
      // The tutorial pointer arrives on its own channel (SOW_tutorial_pointer_update), so keep the last one.
      if (
        hudState.hud &&
        hudState.hud.tutorial &&
        previousHud &&
        previousHud.tutorial
      )
        hudState.hud.tutorial.pointer = previousHud.tutorial.pointer;
      if (
        hudState.hud &&
        hudState.hud.dev_tools &&
        typeof hudState.hud.dev_tools.open === "boolean"
      ) {
        const nextDevOpen = hudState.hud.dev_tools.open;
        if (nextDevOpen && !devSidebarOpen && activeHudPanel !== null) {
          activeHudPanel = null;
        }
        devSidebarOpen = nextDevOpen;
      }
    }
    renderHud();
    if (typeof window.SOW_tutorial_state_update === "function") {
      window.SOW_tutorial_state_update(hudState);
    }
  }

  let lastParsedRaw = "";
  let lastParsedEnvelope: HudStateEnvelope | null = null;
  window.SOW_onStateUpdate = function (raw) {
    if (typeof raw !== "string") return;
    let parsed: HudStateEnvelope;
    if (raw === lastParsedRaw && lastParsedEnvelope !== null) {
      parsed = lastParsedEnvelope;
    } else {
      try {
        parsed = JSON.parse(raw) as HudStateEnvelope;
      } catch (error) {
        console.warn("[WEB STATE] invalid state:", error);
        return;
      }
      lastParsedRaw = raw;
      lastParsedEnvelope = parsed;
    }
    if (typeof window.SOW_menu_state_update === "function") {
      window.SOW_menu_state_update(raw, parsed);
    }
    handleHudStateUpdate(raw, parsed);
  };

  window.SOW_diagnostics_update = function (fps, ping) {
    if (!hudState || hudState.phase !== "Playing" || !hudState.hud) return;
    hudState.hud.fps = fps;
    hudState.hud.ping = ping;
    const settings = hudState.settings || DEFAULT_HUD_SETTINGS;
    renderDiagnostics(fps, ping, settings.show_fps_ping === true);
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
    if (
      !hudState ||
      hudState.phase !== "Playing" ||
      !hudState.hud ||
      !hudState.hud.tutorial
    )
      return;
    hudState.hud.tutorial.pointer = JSON.parse(raw);
    if (typeof window.SOW_tutorial_state_update === "function") {
      window.SOW_tutorial_state_update(hudState);
    }
  };

  if (
    window.SOW_I18N_READY &&
    typeof window.SOW_I18N_READY.then === "function"
  ) {
    window.SOW_I18N_READY.then(function () {
      emoteTextKey = "";
      syncEmoteTexts(hudState && hudState.hud);
    });
  }
  document.addEventListener(
    "keydown",
    function (event) {
      if (!hudInitialized || hudRoot?.hidden || !hudRefs) return;
      if (betrayalOpen || activeHudPanel !== null) return;
      if (
        (hudRefs.surrender && !hudRefs.surrender.classList.contains("hidden")) ||
        (hudRefs.endgame && !hudRefs.endgame.classList.contains("hidden"))
      ) {
        return;
      }
      if (!hudRefs.transfer || hudRefs.transfer.classList.contains("hidden")) {
        const hotkey = /^(?:Digit|Numpad)([0-9])$/.exec(event.code)?.[1];
        if (
          !hotkey ||
          hudState?.phase !== "Playing" ||
          event.defaultPrevented ||
          event.isComposing ||
          event.ctrlKey ||
          event.metaKey ||
          event.altKey ||
          event.shiftKey
        ) {
          return;
        }
        if (
          event.target instanceof Element &&
          event.target.closest(
            "input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='slider'], [role='dialog'], [role='alertdialog']",
          )
        ) {
          return;
        }
        const targetButton = hudRoot?.querySelector<HTMLButtonElement>(
          '.sow-hud__dock-actions button[aria-keyshortcuts="' + hotkey + '"]',
        );
        if (
          !targetButton ||
          targetButton.hidden ||
          targetButton.disabled ||
          !targetButton.getClientRects().length
        ) {
          return;
        }
        event.preventDefault();
        event.stopPropagation();
        if (event.repeat) return;
        targetButton.click();
        return;
      }
      const action = resolveTransferHotkey(event);
      if (!action) return;
      if (action === "close") {
        event.preventDefault();
        event.stopPropagation();
        send("close_transfer");
        return;
      }
      const live = hudState && hudState.hud;
      const offer = live && live.transfer;
      if (!live || !offer) return;
      event.preventDefault();
      event.stopPropagation();
      if (action === "set_max") {
        const target = resolveTransferMaxTarget(
          document.activeElement === hudRefs.transferTroops,
          document.activeElement === hudRefs.transferGold,
        );
        const resources: TransferResource[] =
          target === "both" ? ["troops", "gold"] : [target];
        resources.forEach(function (resource) {
          const input =
            resource === "troops"
              ? hudRefs!.transferTroops
              : hudRefs!.transferGold;
          input.value = String(transferPresetAmount(resource, "max", offer, live));
        });
        return;
      }
      const kind = transferKind(offer, live);
      const cta =
        kind === "send"
          ? hudRefs.transferSend
          : kind === "request"
            ? hudRefs.transferRequest
            : hudRefs.transferTrade;
      if (
        !cta.disabled &&
        (wholeAmount(Number(hudRefs.transferTroops.value)) > 0 ||
          wholeAmount(Number(hudRefs.transferGold.value)) > 0)
      ) {
        cta.click();
      }
    },
    true,
  );
  if (hudRoot)
    hudRoot.addEventListener("keydown", function (event) {
      if (handleHudDropdownKeydown(event)) return;
      if (
        (event.key === "Enter" || event.key === " ") &&
        event.target instanceof HTMLElement &&
        event.target.tagName !== "INPUT" &&
        event.target.tagName !== "TEXTAREA"
      ) {
        const roleButton = event.target.closest<HTMLElement>(
          '[role="button"][data-command]',
        );
        if (roleButton && roleButton.tagName !== "BUTTON") {
          event.preventDefault();
          roleButton.click();
          return;
        }
      }
      if (
        hudRefs &&
        hudRefs.betrayal &&
        !hudRefs.betrayal.classList.contains("hidden")
      ) {
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          pendingCounterAttacks = Object.create(null);
          send("cancel_betrayal");
          return;
        }
        if (event.key === "Tab") {
          const buttons = Array.from(
            hudRefs.betrayal.querySelectorAll<HTMLButtonElement>(
              "button:not(:disabled)",
            ),
          );
          const index = buttons.findIndex(
            (button) => button === document.activeElement,
          );
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
      if (event.key === "Escape" && activeHudPanel === "leaderboard") {
        if (
          document.activeElement === hudRefs?.lbSearch &&
          hudRefs?.lbSearch.value
        ) {
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
      if (
        !menu ||
        menu.classList.contains("hidden") ||
        !menu.contains(event.target as Node)
      )
        return;
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        const back = menu.querySelector<HTMLButtonElement>("[data-map-back]");
        if (back) back.click();
        else send("close_map_context_menu");
      } else if (
        ["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight"].indexOf(
          event.key,
        ) >= 0
      ) {
        // Arrows walk the ring clockwise/counter-clockwise, skipping blocked slots.
        const choices = Array.from(
          menu.querySelectorAll<HTMLButtonElement>(
            ".sow-hud__ring-slot:not(:disabled)",
          ),
        );
        if (!choices.length) return;
        event.preventDefault();
        event.stopPropagation();
        const direction =
          event.key === "ArrowUp" || event.key === "ArrowLeft" ? -1 : 1;
        let index = choices.findIndex(
          (choice) => choice === document.activeElement,
        );
        if (index < 0) index = direction > 0 ? -1 : 0;
        choices[(index + direction + choices.length) % choices.length].focus({
          preventScroll: true,
        });
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
      if (
        menu &&
        target instanceof HTMLElement &&
        menu.contains(target) &&
        target.dataset.slot != null
      )
        setRingHot(menu, Number(target.dataset.slot));
    });

  if (window.SOW_INITIAL_LOADER_READY === true)
    activateDeferredBuildingAtlases();
  else
    window.addEventListener(
      "sow:loader-ready",
      activateDeferredBuildingAtlases,
      { once: true },
    );

  window.addEventListener("sow:locale-change", function () {
    emoteTextKey = "";
    if (!hudState) return;
    const active = document.activeElement;
    const transferDraft =
      hudRefs &&
      hudRefs.transfer &&
      !hudRefs.transfer.classList.contains("hidden")
        ? {
            target: hudRefs.transfer.dataset.targetId,
            gold: hudRefs.transferGold!.value,
            troops: hudRefs.transferTroops!.value,
            flips: { troops: transferFlips.troops, gold: transferFlips.gold },
            focus: hudRefs.transfer.contains(active) ? active!.id : null,
            returnFocus: hudRefs.transferReturnFocus,
          }
        : null;
    const betrayalFocus =
      hudRefs && hudRefs.betrayal && hudRefs.betrayal.contains(active)
        ? active!.id
        : null;
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
    const cachedLbPlayers = lastLeaderboardPlayersPayload;
    const cachedLbTeams = lastLeaderboardTeamsPayload;
    cancelLeaderboardWork();
    leaderboardRows = Object.create(null);
    leaderboardTeamRows = Object.create(null);
    leaderboardRenderKey = "";
    lastLeaderboardPlayersPayload = null;
    lastLeaderboardTeamsPayload = null;
    cachedIntegerLocale = "";
    cachedIntegerFormatter = null;
    inboxDataVersion++;
    requestToastStateVersion++;
    requestToastRenderKey = "";
    lastRequestToastsVisible = null;
    lastRequestToastsStackCount = -1;
    clearLeavingRequestToasts();
    if (hudRoot) hudRoot.replaceChildren();
    if (hudDevLayer && hudDevLayer.parentNode) {
      const freshDevLayer = hudDevLayer.cloneNode(false) as HTMLElement;
      hudDevLayer.parentNode.replaceChild(freshDevLayer, hudDevLayer);
      hudDevLayer = freshDevLayer;
    }
    renderHud(true);
    if (
      activeHudPanel === "leaderboard" &&
      !Array.isArray(lastLeaderboardPlayersPayload) &&
      Array.isArray(cachedLbPlayers)
    ) {
      const teams = cachedLbTeams || lastLeaderboardTeams;
      lastLeaderboardPlayersPayload = cachedLbPlayers;
      lastLeaderboardTeamsPayload = teams;
      updateLeaderboard(cachedLbPlayers, teams);
    }
    const refreshedRefs = hudRefs as HudRefs | null;
    if (
      transferDraft &&
      refreshedRefs &&
      refreshedRefs.transfer.dataset.targetId === transferDraft.target
    ) {
      transferFlips.troops = transferDraft.flips.troops;
      transferFlips.gold = transferDraft.flips.gold;
      renderHud();
      refreshedRefs.transferGold.value = transferDraft.gold;
      refreshedRefs.transferTroops.value = transferDraft.troops;
      refreshedRefs.transferReturnFocus = transferDraft.returnFocus;
      if (transferDraft.focus)
        restoreHudFocus(document.getElementById(transferDraft.focus));
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
  window.SOW_getDevSettingsExport = getDevSettingsExport;
  if (typeof window.SOW_MENU_STATE === "string")
    window.SOW_onStateUpdate(window.SOW_MENU_STATE);
})();
