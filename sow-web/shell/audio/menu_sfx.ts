/**
 * Subtle AAA Menu & HUD Micro-Interaction Audio Controller (`menu_sfx.ts`).
 *
 * ARCHITECTURE
 * ============
 * Centralizes hover, pointer-down press, and command-level UI audio so Battle
 * menus, submenus, Hero browsing, Hero Info panel tabbing, and Purchase flows
 * stay cohesive, subtle, and free of double-triggered sounds:
 *
 *   1. Two-part physical switch feel:
 *        `pointerover` (mouse/pen only) -> feather-light `UiHover`
 *        `pointerdown` (all pointers)   -> damped downward `UiPress`
 *        `click` / command              -> crisp semantic release cue
 *   2. Zero hover re-triggering inside child spans/icons via per-surface
 *      `lastHovered` tracking.
 *   3. Screen-transition deduplication (`suppressNextScreenSwitchSound` /
 *      `consumeScreenSwitchSoundSuppression`) ensures bespoke cues like
 *      `UiBattleLaunch` or `UiHeroEquip` never stack on top of `render()`'s
 *      automatic `UiTabSwitch`.
 */

import { audioEngine } from "./engine.ts";
import {
    AudioCue,
    UiBattleLaunchSubtype,
    UiHeroBrowseSubtype,
    UiHoverSubtype,
    UiInfoTabSubtype,
    UiPressSubtype,
    UiPurchaseActionSubtype,
} from "./types.ts";

const INTERACTIVE_SELECTOR = [
    "button",
    "[data-command]",
    "[role='button']",
    "[role='tab']",
    "[data-role='dropdown-option']",
    ".sow-control-dropdown__option",
    "input[type='checkbox']",
    "[data-placement-action]",
    "[data-placement-nudge]",
    "[data-production-kind]",
    "[data-production-max]",
    "[data-fleet-order]",
    "[data-map-action]",
    "[data-map-group]",
    "[data-map-back]",
    "[data-emote-kind]",
].join(", ");

const PRIMARY_COMMANDS = new Set([
    "quick_match",
    "join_matchmaking",
    "start_campaign_episode",
    "start_private",
    "create_game",
    "confirm_purchase",
    "claim_mission",
    "claim_mission_double",
    "claim_campaign_hero",
    "buy_product",
]);

const PURCHASE_COMMANDS = new Set([
    "unlock_leader",
    "open_skin_purchase",
    "open_offer_purchase",
    "portal_unlock_leader",
    "confirm_purchase",
    "buy_product",
]);

const installedSurfaces = new WeakSet<HTMLElement>();
let screenSwitchSuppressed = false;

/** Suppresses the next automatic `render()` `UiTabSwitch` when a bespoke cue already fired. */
export function suppressNextScreenSwitchSound(): void {
    screenSwitchSuppressed = true;
}

/** Consumes and returns whether the upcoming `render()` screen-switch sound is suppressed. */
export function consumeScreenSwitchSoundSuppression(): boolean {
    if (!screenSwitchSuppressed) return false;
    screenSwitchSuppressed = false;
    return true;
}

function isDisabledControl(el: Element): boolean {
    if (el instanceof HTMLButtonElement || el instanceof HTMLInputElement) {
        if (el.disabled) return true;
    }
    return el.getAttribute("aria-disabled") === "true" || el.hasAttribute("disabled");
}

function isPrimaryControl(el: HTMLElement): boolean {
    const cmd = el.dataset.command || "";
    if (PRIMARY_COMMANDS.has(cmd)) return true;
    return (
        el.classList.contains("sow-menu__primary") ||
        el.classList.contains("sow-store__buy--primary") ||
        el.classList.contains("is-confirm")
    );
}

function isSoftControl(el: HTMLElement): boolean {
    const cmd = el.dataset.command || "";
    if (
        cmd.startsWith("close_") ||
        cmd.startsWith("cancel_") ||
        cmd === "leave_lobby" ||
        cmd === "unequip_skin"
    ) {
        return true;
    }
    return (
        el.classList.contains("sow-menu__icon-button") ||
        el.classList.contains("sow-menu__ghost-button") ||
        el.classList.contains("sow-menu__danger") ||
        el.classList.contains("is-cancel") ||
        el.hasAttribute("data-map-back")
    );
}

function isCardControl(el: HTMLElement): boolean {
    return (
        el.classList.contains("sow-heroes__card") ||
        el.classList.contains("sow-menu__lobby") ||
        el.classList.contains("sow-campaign__episode") ||
        el.classList.contains("sow-store__card") ||
        el.classList.contains("sow-store__skin-card") ||
        el.classList.contains("sow-directive-card") ||
        el.classList.contains("sow-profile__ach-pillar-card")
    );
}

/** Normalized cyclic sibling index in [0..1] for gentle melodic glide across grids & tab bars. */
export function siblingSlotRatio(el: Element): number {
    const parent = el.parentElement;
    if (!parent) return 0.5;
    let index = 0;
    let count = 0;
    for (let child = parent.firstElementChild; child; child = child.nextElementSibling) {
        if (child === el) index = count;
        count++;
    }
    if (count <= 1) return 0.5;
    // Wrap smoothly across 8 steps so large rosters glide cleanly without hitting pitch ceilings
    const span = Math.min(count - 1, 7);
    return (index % (span + 1)) / Math.max(1, span);
}

/**
 * Installs subtle delegated `UiHover` (mouse/pen only) and `UiPress` (all pointers)
 * listeners on a UI root element (`#sow-menu` or `#sow-hud`). Safe to call repeatedly.
 */
export function installSurfaceHoverPressAudio(surface: HTMLElement | null | undefined): void {
    if (!surface || installedSurfaces.has(surface)) return;
    installedSurfaces.add(surface);

    let lastHovered: HTMLElement | null = null;

    surface.addEventListener(
        "pointerover",
        (event) => {
            if (event.pointerType === "touch") return;
            const raw = event.target instanceof Element ? event.target : null;
            const el = raw?.closest<HTMLElement>(INTERACTIVE_SELECTOR) || null;
            if (!el || !surface.contains(el)) {
                lastHovered = null;
                return;
            }
            if (el === lastHovered) return;
            lastHovered = el;
            if (isDisabledControl(el)) return;

            if (isCardControl(el)) {
                audioEngine.playUi(AudioCue.UiHover, UiHoverSubtype.Card, siblingSlotRatio(el));
            } else if (isPrimaryControl(el)) {
                audioEngine.playUi(AudioCue.UiHover, UiHoverSubtype.Primary, 0.55);
            } else {
                audioEngine.playUi(AudioCue.UiHover, UiHoverSubtype.Standard, 0.4);
            }
        },
        { passive: true },
    );

    surface.addEventListener(
        "pointerout",
        (event) => {
            if (!lastHovered) return;
            const related = event.relatedTarget instanceof Node ? event.relatedTarget : null;
            if (!related || !lastHovered.contains(related)) {
                lastHovered = null;
            }
        },
        { passive: true },
    );

    surface.addEventListener(
        "pointerdown",
        (event) => {
            const raw = event.target instanceof Element ? event.target : null;
            if (!raw) return;
            // Skip text/search/range inputs so typing & slider dragging stay clean
            if (
                raw instanceof HTMLInputElement &&
                raw.type !== "checkbox" &&
                raw.type !== "button" &&
                raw.type !== "submit"
            ) {
                return;
            }
            if (raw instanceof HTMLTextAreaElement || raw instanceof HTMLSelectElement) {
                return;
            }
            const el = raw.closest<HTMLElement>(INTERACTIVE_SELECTOR);
            if (!el || !surface.contains(el)) return;

            if (isDisabledControl(el)) {
                const cmd = el.dataset.command || "";
                if (PURCHASE_COMMANDS.has(cmd)) {
                    audioEngine.playUi(AudioCue.UiPurchaseAction, UiPurchaseActionSubtype.Denied);
                }
                return;
            }

            if (isPrimaryControl(el)) {
                audioEngine.playUi(AudioCue.UiPress, UiPressSubtype.Primary, 0.6);
            } else if (isSoftControl(el)) {
                audioEngine.playUi(AudioCue.UiPress, UiPressSubtype.Soft, 0.45);
            } else {
                audioEngine.playUi(AudioCue.UiPress, UiPressSubtype.Standard, 0.5);
            }
        },
        { passive: true },
    );
}

/** Semantic helpers for clean, self-documenting call sites in `main_menu.*.ts`. */
export const MenuSfx = {
    heroBrowse(mode: "available" | "locked" | "random", target?: Element | null): void {
        const sub =
            mode === "locked"
                ? UiHeroBrowseSubtype.Locked
                : mode === "random"
                  ? UiHeroBrowseSubtype.Random
                  : UiHeroBrowseSubtype.Available;
        audioEngine.playUi(AudioCue.UiHeroBrowse, sub, target ? siblingSlotRatio(target) : 0.5);
    },

    infoTab(ratioOrTarget?: number | Element | null): void {
        const ratio =
            typeof ratioOrTarget === "number"
                ? ratioOrTarget
                : ratioOrTarget
                  ? siblingSlotRatio(ratioOrTarget)
                  : 0.5;
        audioEngine.playUi(AudioCue.UiInfoTab, UiInfoTabSubtype.Tab, ratio);
    },

    filterChip(target?: Element | null): void {
        audioEngine.playUi(
            AudioCue.UiInfoTab,
            UiInfoTabSubtype.Chip,
            target ? siblingSlotRatio(target) : 0.5,
        );
    },

    toggleSwitch(checked: boolean): void {
        audioEngine.playUi(AudioCue.UiInfoTab, UiInfoTabSubtype.Switch, checked ? 1 : 0);
    },

    purchaseOpen(): void {
        audioEngine.playUi(AudioCue.UiPurchaseAction, UiPurchaseActionSubtype.Open);
    },

    purchaseConfirm(): void {
        audioEngine.playUi(AudioCue.UiPurchaseAction, UiPurchaseActionSubtype.Confirm);
    },

    purchaseDenied(): void {
        audioEngine.playUi(AudioCue.UiPurchaseAction, UiPurchaseActionSubtype.Denied);
    },

    battleSubmenu(): void {
        suppressNextScreenSwitchSound();
        audioEngine.playUi(AudioCue.UiBattleLaunch, UiBattleLaunchSubtype.Submenu);
    },

    battleLaunch(): void {
        suppressNextScreenSwitchSound();
        audioEngine.playUi(AudioCue.UiBattleLaunch, UiBattleLaunchSubtype.Launch);
    },

    tactileStep(): void {
        audioEngine.playUi(AudioCue.UiBattleLaunch, UiBattleLaunchSubtype.TactileStep);
    },

    modalOpen(): void {
        audioEngine.playUi(AudioCue.UiModalOpen);
    },

    modalClose(): void {
        audioEngine.playUi(AudioCue.UiModalClose);
    },

    heroEquip(): void {
        suppressNextScreenSwitchSound();
        audioEngine.playUi(AudioCue.UiHeroEquip);
    },

    back(): void {
        suppressNextScreenSwitchSound();
        audioEngine.playUi(AudioCue.UiClick, 2);
    },

    confirm(): void {
        audioEngine.playUi(AudioCue.UiClick, 1);
    },

    click(): void {
        audioEngine.playUi(AudioCue.UiClick, 0);
    },
};
