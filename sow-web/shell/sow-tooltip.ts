export type SowTooltipTone = "default" | "gold" | "muted" | "gem" | "emerald" | "danger";
export type SowTooltipPlacement = "auto" | "top" | "bottom" | "start" | "end";

export interface SowTooltipBadge {
    label: string;
    tone?: SowTooltipTone;
}

export interface SowTooltipMetaItem {
    label: string;
    value: string;
    tone?: SowTooltipTone;
}

export interface SowTooltipAction {
    id?: string;
    label: string;
    command?: string;
    dataset?: Record<string, string>;
    tone?: "primary" | "secondary" | "danger";
    dismiss?: boolean;
    onClick?: (anchor: HTMLElement, event: MouseEvent) => void;
}

export type SowTooltipIcon =
    | string
    | {
          name?: string;
          svg?: string;
          imageUrl?: string;
      };

export interface SowTooltipSpec {
    text?: string;
    title?: string;
    eyebrow?: string;
    icon?: SowTooltipIcon;
    badge?: SowTooltipBadge;
    meta?: SowTooltipMetaItem[];
    actions?: SowTooltipAction[];
    tone?: SowTooltipTone;
    placement?: SowTooltipPlacement;
    maxWidth?: number;
    interactive?: boolean;
}

export type SowTooltipResolver =
    | SowTooltipSpec
    | string
    | ((anchor: HTMLElement) => SowTooltipSpec | string | null | undefined);

const TOOLTIP_ID_ATTR = "data-sow-tooltip-id";
const TOOLTIP_TEXT_ATTR = "data-sow-tooltip";
const TOOLTIP_SELECTOR = `[${TOOLTIP_TEXT_ATTR}], [${TOOLTIP_ID_ATTR}]`;
const PORTAL_ELEMENT_ID = "sow-tooltip-portal";
const VIEWPORT_MARGIN = 10;
const TRIGGER_GAP = 10;
const SHOW_DELAY_MS = 95;
const SWITCH_DELAY_MS = 35;
const HIDE_DELAY_MS = 140;

const BUILTIN_ICONS: Record<string, string> = {
    lock: "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><rect x='5' y='11' width='14' height='10' rx='2'/><path d='M8 11V7a4 4 0 0 1 8 0v4'/></svg>",
    info: "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='9'/><line x1='12' y1='16' x2='12' y2='12'/><line x1='12' y1='8' x2='12.01' y2='8'/></svg>",
    swap: "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><polyline points='17 1 21 5 17 9'/><path d='M3 11V9a4 4 0 0 1 4-4h14'/><polyline points='7 23 3 19 7 15'/><path d='M21 13v2a4 4 0 0 1-4 4H3'/></svg>",
    play: "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2.1' stroke-linecap='round' stroke-linejoin='round'><polygon points='6 4 20 12 6 20 6 4'/></svg>",
    ranked: "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M6 9H4.5a2.5 2.5 0 0 1 0-5H6'/><path d='M18 9h1.5a2.5 2.5 0 0 0 0-5H18'/><path d='M4 22h16'/><path d='M10 14.66V17c0 .55-.47.98-.97 1.21C7.85 18.75 7 20.24 7 22'/><path d='M14 14.66V17c0 .55.47.98.97 1.21C16.15 18.75 17 20.24 17 22'/><path d='M18 2H6v7a6 6 0 0 0 12 0V2Z'/></svg>",
};

const registry: Record<string, SowTooltipResolver> = Object.create(null);
const inlineSpecs = new WeakMap<HTMLElement, SowTooltipSpec>();

let initialized = false;
let portal: HTMLDivElement | null = null;
let anchorEl: HTMLElement | null = null;
let pendingAnchor: HTMLElement | null = null;
let currentSpec: SowTooltipSpec | null = null;
let lastHideAt = 0;
let showTimer: number | null = null;
let hideTimer: number | null = null;
let domObserver: MutationObserver | null = null;
let observedDescribedByEl: HTMLElement | null = null;
let currentPlacement: "top" | "bottom" | "left" | "right" = "top";

function escHtml(value: unknown): string {
    return String(value == null ? "" : value)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
}

function isRtl(): boolean {
    return typeof document !== "undefined" && document.documentElement.dir === "rtl";
}

function normalizeTone(value: string | undefined): SowTooltipTone {
    if (
        value === "gold" ||
        value === "muted" ||
        value === "gem" ||
        value === "emerald" ||
        value === "danger"
    ) {
        return value;
    }
    return "default";
}

function normalizePlacement(value: string | undefined): SowTooltipPlacement {
    if (
        value === "top" ||
        value === "bottom" ||
        value === "start" ||
        value === "end"
    ) {
        return value;
    }
    return "auto";
}

function resolveIconMarkup(icon: SowTooltipIcon | undefined): string {
    if (!icon) return "";
    if (typeof icon === "string") {
        const key = icon.trim();
        if (!key) return "";
        if (BUILTIN_ICONS[key]) return BUILTIN_ICONS[key];
        if (typeof window !== "undefined" && typeof window.SOW_hudIcon === "function") {
            return window.SOW_hudIcon(key) || "";
        }
        return "";
    }
    if (icon.imageUrl) {
        return `<img class="sow-tooltip__icon-img" src="${escHtml(icon.imageUrl)}" alt="" aria-hidden="true">`;
    }
    if (icon.svg) return icon.svg;
    if (icon.name) return resolveIconMarkup(icon.name);
    return "";
}

function normalizeSpec(raw: SowTooltipSpec | string | null | undefined): SowTooltipSpec | null {
    if (raw == null) return null;
    if (typeof raw === "string") {
        const trimmed = raw.trim();
        return trimmed ? { text: trimmed } : null;
    }
    const text = raw.text?.trim() || "";
    const title = raw.title?.trim() || "";
    const eyebrow = raw.eyebrow?.trim() || "";
    const hasMeta = Array.isArray(raw.meta) && raw.meta.length > 0;
    const hasActions = Array.isArray(raw.actions) && raw.actions.length > 0;
    if (!text && !title && !eyebrow && !hasMeta && !hasActions) {
        return null;
    }
    return {
        ...raw,
        text: text || undefined,
        title: title || undefined,
        eyebrow: eyebrow || undefined,
        tone: normalizeTone(raw.tone),
        placement: normalizePlacement(raw.placement),
    };
}

function readSpecFromElement(el: HTMLElement): SowTooltipSpec | null {
    const inline = inlineSpecs.get(el);
    if (inline) return normalizeSpec(inline);

    const specId = el.getAttribute(TOOLTIP_ID_ATTR);
    if (specId && registry[specId]) {
        const entry = registry[specId];
        const evaluated = typeof entry === "function" ? entry(el) : entry;
        const resolved = normalizeSpec(evaluated);
        if (resolved) return resolved;
    }

    const text = el.getAttribute(TOOLTIP_TEXT_ATTR) || "";
    const title = el.getAttribute("data-sow-tooltip-title") || "";
    const eyebrow = el.getAttribute("data-sow-tooltip-eyebrow") || "";
    const icon = el.getAttribute("data-sow-tooltip-icon") || "";
    const badgeLabel = el.getAttribute("data-sow-tooltip-badge") || "";
    const badgeTone = normalizeTone(el.getAttribute("data-sow-tooltip-badge-tone") || undefined);
    const tone = normalizeTone(el.getAttribute("data-sow-tooltip-tone") || undefined);
    const placement = normalizePlacement(el.getAttribute("data-sow-tooltip-placement") || undefined);

    return normalizeSpec({
        text,
        title,
        eyebrow,
        icon: icon || undefined,
        badge: badgeLabel ? { label: badgeLabel, tone: badgeTone } : undefined,
        tone,
        placement,
    });
}

function clearTimers(): void {
    if (showTimer !== null) {
        window.clearTimeout(showTimer);
        showTimer = null;
    }
    if (hideTimer !== null) {
        window.clearTimeout(hideTimer);
        hideTimer = null;
    }
}

function clearDescribedBy(): void {
    if (!observedDescribedByEl) return;
    if (observedDescribedByEl.getAttribute("aria-describedby") === PORTAL_ELEMENT_ID) {
        observedDescribedByEl.removeAttribute("aria-describedby");
    }
    observedDescribedByEl = null;
}

function ensurePortal(): HTMLDivElement {
    if (portal && portal.isConnected) return portal;
    const node = document.createElement("div");
    node.id = PORTAL_ELEMENT_ID;
    node.className = "sow-tooltip";
    node.setAttribute("role", "tooltip");
    node.hidden = true;
    node.addEventListener("pointerenter", function () {
        if (currentSpec?.interactive || currentSpec?.actions?.length) {
            if (hideTimer !== null) {
                window.clearTimeout(hideTimer);
                hideTimer = null;
            }
        }
    });
    node.addEventListener("pointerleave", function () {
        if (currentSpec?.interactive || currentSpec?.actions?.length) {
            scheduleHide(HIDE_DELAY_MS);
        }
    });
    node.addEventListener("click", handleTooltipActionClick);
    document.body.appendChild(node);
    portal = node;
    return node;
}

function dispatchHostCommand(
    anchor: HTMLElement | null,
    command: string,
    dataset?: Record<string, string>,
): void {
    const scope =
        (anchor && anchor.closest<HTMLElement>("#sow-hud, #sow-menu")) ||
        document.getElementById("sow-hud") ||
        document.getElementById("sow-menu") ||
        document.body;
    const proxy = document.createElement("button");
    proxy.type = "button";
    proxy.hidden = true;
    proxy.style.display = "none";
    proxy.dataset.command = command;
    if (dataset) {
        for (const [key, value] of Object.entries(dataset)) {
            proxy.dataset[key] = value;
        }
    }
    scope.appendChild(proxy);
    proxy.click();
    proxy.remove();
}

function handleTooltipActionClick(event: MouseEvent): void {
    const target = event.target as HTMLElement | null;
    const btn = target?.closest<HTMLButtonElement>("[data-sow-tooltip-action]");
    if (!btn || !currentSpec?.actions?.length) return;
    const index = Number(btn.dataset.sowTooltipAction);
    const action = currentSpec.actions[index];
    if (!action) return;
    event.preventDefault();
    event.stopPropagation();

    const trigger = anchorEl;
    if (typeof action.onClick === "function" && trigger) {
        action.onClick(trigger, event);
    } else if (action.command) {
        dispatchHostCommand(trigger, action.command, action.dataset);
    }
    if (action.dismiss !== false) {
        hide(true);
    }
}

function renderTooltipContent(node: HTMLDivElement, spec: SowTooltipSpec): void {
    const tone = normalizeTone(spec.tone);
    const isRich = Boolean(
        spec.title || spec.eyebrow || spec.icon || spec.badge || spec.meta?.length || spec.actions?.length,
    );
    const interactive = Boolean(spec.interactive || spec.actions?.length);

    node.dataset.tone = tone;
    node.dataset.rich = isRich ? "true" : "false";
    node.dataset.interactive = interactive ? "true" : "false";
    node.setAttribute("role", interactive ? "dialog" : "tooltip");
    node.style.maxWidth = `${Math.max(160, Number(spec.maxWidth) || 292)}px`;

    const iconMarkup = resolveIconMarkup(spec.icon);
    const hasHeader = Boolean(spec.title || spec.eyebrow || iconMarkup || spec.badge);

    let html = "<div class='sow-tooltip__surface'>";

    if (hasHeader) {
        html += "<div class='sow-tooltip__head'>";
        if (iconMarkup) {
            html += `<span class='sow-tooltip__icon' aria-hidden='true'>${iconMarkup}</span>`;
        }
        html += "<div class='sow-tooltip__head-copy'>";
        if (spec.eyebrow) {
            html += `<span class='sow-tooltip__eyebrow'>${escHtml(spec.eyebrow)}</span>`;
        }
        if (spec.title) {
            html += `<strong class='sow-tooltip__title'>${escHtml(spec.title)}</strong>`;
        }
        html += "</div>";
        if (spec.badge?.label) {
            const bTone = normalizeTone(spec.badge.tone || spec.tone);
            html += `<span class='sow-tooltip__badge' data-tone='${bTone}'>${escHtml(spec.badge.label)}</span>`;
        }
        html += "</div>";
    }

    if (spec.text) {
        html += `<p class='sow-tooltip__text'>${escHtml(spec.text)}</p>`;
    }

    if (spec.meta?.length) {
        html += "<div class='sow-tooltip__meta'>";
        for (const item of spec.meta) {
            const mTone = normalizeTone(item.tone);
            html +=
                `<div class='sow-tooltip__meta-row' data-tone='${mTone}'>` +
                `<span class='sow-tooltip__meta-label'>${escHtml(item.label)}</span>` +
                `<strong class='sow-tooltip__meta-val'>${escHtml(item.value)}</strong>` +
                "</div>";
        }
        html += "</div>";
    }

    if (spec.actions?.length) {
        html += "<div class='sow-tooltip__actions'>";
        spec.actions.forEach(function (action, idx) {
            const btnTone = action.tone || (idx === 0 ? "primary" : "secondary");
            html +=
                `<button type='button' class='sow-tooltip__action sow-tooltip__action--${btnTone}' data-sow-tooltip-action='${idx}'>` +
                `${escHtml(action.label)}</button>`;
        });
        html += "</div>";
    }

    html += "</div><span class='sow-tooltip__arrow' aria-hidden='true'></span>";
    node.innerHTML = html;
}

function positionPortal(node: HTMLDivElement, trigger: HTMLElement, spec: SowTooltipSpec): void {
    const rect = trigger.getBoundingClientRect();
    const vw = document.documentElement.clientWidth || window.innerWidth;
    const vh = document.documentElement.clientHeight || window.innerHeight;
    if (rect.width === 0 && rect.height === 0) {
        hide(true);
        return;
    }

    // Measure natural size offscreen-safe before placing
    node.style.left = "0px";
    node.style.top = "0px";
    node.hidden = false;
    const tipRect = node.getBoundingClientRect();
    const w = tipRect.width;
    const h = tipRect.height;

    const preferred = spec.placement || "auto";
    const rtl = isRtl();
    let physical: "top" | "bottom" | "left" | "right" = "top";
    if (preferred === "bottom") physical = "bottom";
    else if (preferred === "start") physical = rtl ? "right" : "left";
    else if (preferred === "end") physical = rtl ? "left" : "right";
    else physical = "top";

    const spaceTop = rect.top - VIEWPORT_MARGIN;
    const spaceBottom = vh - rect.bottom - VIEWPORT_MARGIN;
    const spaceLeft = rect.left - VIEWPORT_MARGIN;
    const spaceRight = vw - rect.right - VIEWPORT_MARGIN;

    if (preferred === "auto") {
        if (spaceTop >= h + TRIGGER_GAP) physical = "top";
        else if (spaceBottom >= h + TRIGGER_GAP) physical = "bottom";
        else if (spaceRight >= w + TRIGGER_GAP) physical = "right";
        else if (spaceLeft >= w + TRIGGER_GAP) physical = "left";
        else physical = spaceBottom >= spaceTop ? "bottom" : "top";
    } else if (physical === "top" && spaceTop < h + TRIGGER_GAP && spaceBottom >= spaceTop) {
        physical = "bottom";
    } else if (physical === "bottom" && spaceBottom < h + TRIGGER_GAP && spaceTop > spaceBottom) {
        physical = "top";
    } else if (physical === "left" && spaceLeft < w + TRIGGER_GAP && spaceRight > spaceLeft) {
        physical = "right";
    } else if (physical === "right" && spaceRight < w + TRIGGER_GAP && spaceLeft > spaceRight) {
        physical = "left";
    }

    currentPlacement = physical;
    node.dataset.placement = physical;

    const triggerCx = rect.left + rect.width / 2;
    const triggerCy = rect.top + rect.height / 2;
    let x = 0;
    let y = 0;

    if (physical === "top") {
        x = triggerCx - w / 2;
        y = rect.top - h - TRIGGER_GAP;
    } else if (physical === "bottom") {
        x = triggerCx - w / 2;
        y = rect.bottom + TRIGGER_GAP;
    } else if (physical === "left") {
        x = rect.left - w - TRIGGER_GAP;
        y = triggerCy - h / 2;
    } else {
        x = rect.right + TRIGGER_GAP;
        y = triggerCy - h / 2;
    }

    x = Math.max(VIEWPORT_MARGIN, Math.min(x, vw - w - VIEWPORT_MARGIN));
    y = Math.max(VIEWPORT_MARGIN, Math.min(y, vh - h - VIEWPORT_MARGIN));

    node.style.left = `${Math.round(x)}px`;
    node.style.top = `${Math.round(y)}px`;

    // Align arrow squarely with trigger center within tooltip bounds
    const arrowMargin = 14;
    if (physical === "top" || physical === "bottom") {
        const arrowX = Math.max(arrowMargin, Math.min(w - arrowMargin, triggerCx - x));
        node.style.setProperty("--sow-tooltip-arrow-offset", `${Math.round(arrowX)}px`);
    } else {
        const arrowY = Math.max(arrowMargin, Math.min(h - arrowMargin, triggerCy - y));
        node.style.setProperty("--sow-tooltip-arrow-offset", `${Math.round(arrowY)}px`);
    }
}

export function show(trigger: HTMLElement, explicitSpec?: SowTooltipSpec | string): void {
    init();
    if (!trigger || !trigger.isConnected) return;
    const spec = normalizeSpec(explicitSpec) || readSpecFromElement(trigger);
    if (!spec) {
        hide(true);
        return;
    }

    clearTimers();
    const node = ensurePortal();
    anchorEl = trigger;
    pendingAnchor = null;
    currentSpec = spec;

    if (observedDescribedByEl !== trigger) {
        clearDescribedBy();
        if (!spec.interactive && !spec.actions?.length) {
            trigger.setAttribute("aria-describedby", PORTAL_ELEMENT_ID);
            observedDescribedByEl = trigger;
        }
    }

    renderTooltipContent(node, spec);
    positionPortal(node, trigger, spec);
    node.classList.add("is-visible");
    startOrphanObserver();
}

export function hide(immediate = false): void {
    clearTimers();
    pendingAnchor = null;
    if (!portal || portal.hidden) {
        anchorEl = null;
        currentSpec = null;
        clearDescribedBy();
        return;
    }
    const doHide = function () {
        if (!portal) return;
        portal.classList.remove("is-visible");
        portal.hidden = true;
        portal.innerHTML = "";
        anchorEl = null;
        currentSpec = null;
        lastHideAt = Date.now();
        clearDescribedBy();
        stopOrphanObserver();
    };
    if (immediate) {
        doHide();
        return;
    }
    scheduleHide(HIDE_DELAY_MS);
}

function scheduleShow(trigger: HTMLElement, immediate = false): void {
    if (!trigger.isConnected) return;
    const spec = readSpecFromElement(trigger);
    if (!spec) return;
    clearTimers();
    pendingAnchor = trigger;
    if (immediate) {
        show(trigger, spec);
        return;
    }
    const delay =
        (portal && !portal.hidden) || Date.now() - lastHideAt < 180 ? SWITCH_DELAY_MS : SHOW_DELAY_MS;
    showTimer = window.setTimeout(function () {
        showTimer = null;
        if (pendingAnchor && pendingAnchor.isConnected) {
            show(pendingAnchor);
        }
    }, delay);
}

function scheduleHide(delay = HIDE_DELAY_MS): void {
    if (hideTimer !== null) window.clearTimeout(hideTimer);
    hideTimer = window.setTimeout(function () {
        hideTimer = null;
        hide(true);
    }, delay);
}

function stopOrphanObserver(): void {
    if (domObserver) {
        domObserver.disconnect();
        domObserver = null;
    }
}

function startOrphanObserver(): void {
    if (domObserver || typeof MutationObserver !== "function") return;
    domObserver = new MutationObserver(function () {
        if (anchorEl && !anchorEl.isConnected) {
            hide(true);
            return;
        }
        if (anchorEl && portal && !portal.hidden && currentSpec) {
            positionPortal(portal, anchorEl, currentSpec);
        }
    });
    const host = anchorEl?.closest("#sow-menu, #sow-hud") || document.body;
    domObserver.observe(host, { childList: true, subtree: true });
}

function findTooltipTrigger(node: EventTarget | null): HTMLElement | null {
    const el = node instanceof Element ? node : null;
    return el ? el.closest<HTMLElement>(TOOLTIP_SELECTOR) : null;
}

export function register(id: string, resolver: SowTooltipResolver): void {
    if (id) registry[id] = resolver;
}

export function set(el: Element | null | undefined, spec: SowTooltipSpec | string | null | undefined): void {
    if (!(el instanceof HTMLElement)) return;
    el.removeAttribute("title");
    if (spec == null || (typeof spec === "string" && !spec.trim())) {
        inlineSpecs.delete(el);
        el.removeAttribute(TOOLTIP_TEXT_ATTR);
        if (anchorEl === el) hide(true);
        return;
    }
    if (typeof spec === "string") {
        inlineSpecs.delete(el);
        const trimmed = spec.trim();
        if (el.getAttribute(TOOLTIP_TEXT_ATTR) !== trimmed) {
            el.setAttribute(TOOLTIP_TEXT_ATTR, trimmed);
        }
    } else {
        const normalized = normalizeSpec(spec);
        if (!normalized) {
            inlineSpecs.delete(el);
            el.removeAttribute(TOOLTIP_TEXT_ATTR);
            if (anchorEl === el) hide(true);
            return;
        }
        inlineSpecs.set(el, normalized);
        el.setAttribute(TOOLTIP_TEXT_ATTR, normalized.text || normalized.title || "1");
    }
    if (anchorEl === el && portal && !portal.hidden) {
        show(el);
    }
}

export function clear(el: Element | null | undefined): void {
    set(el, null);
}

export function init(): void {
    if (initialized || typeof document === "undefined") return;
    initialized = true;

    document.addEventListener(
        "pointerover",
        function (event) {
            if (event.pointerType === "touch") return;
            const target = event.target as HTMLElement | null;
            if (portal && target && portal.contains(target)) {
                if (hideTimer !== null) {
                    window.clearTimeout(hideTimer);
                    hideTimer = null;
                }
                return;
            }
            const trigger = findTooltipTrigger(event.target);
            if (!trigger) return;
            if (anchorEl === trigger && portal && !portal.hidden) {
                if (hideTimer !== null) {
                    window.clearTimeout(hideTimer);
                    hideTimer = null;
                }
                return;
            }
            scheduleShow(trigger, false);
        },
        true,
    );

    document.addEventListener(
        "pointerout",
        function (event) {
            if (event.pointerType === "touch") return;
            const trigger = findTooltipTrigger(event.target);
            if (!trigger) return;
            const next = event.relatedTarget as HTMLElement | null;
            if (next && (trigger.contains(next) || (portal && portal.contains(next)))) {
                return;
            }
            if (pendingAnchor === trigger) {
                clearTimers();
                pendingAnchor = null;
            }
            if (anchorEl === trigger) {
                const interactive = Boolean(currentSpec?.interactive || currentSpec?.actions?.length);
                scheduleHide(interactive ? HIDE_DELAY_MS : 65);
            }
        },
        true,
    );

    document.addEventListener(
        "focusin",
        function (event) {
            const trigger = findTooltipTrigger(event.target);
            if (!trigger) return;
            // Avoid popups from mouse click focus unless keyboard-navigated or info badge
            const isInfoTrigger =
                trigger.hasAttribute("data-sow-tooltip-tap") ||
                trigger.matches("[role='button'], button, a, input, [tabindex]");
            if (!isInfoTrigger) return;
            if (typeof trigger.matches === "function" && trigger.matches(":focus-visible")) {
                scheduleShow(trigger, true);
            }
        },
        true,
    );

    document.addEventListener(
        "focusout",
        function (event) {
            const trigger = findTooltipTrigger(event.target);
            if (!trigger || anchorEl !== trigger) return;
            const next = event.relatedTarget as HTMLElement | null;
            if (next && portal && portal.contains(next)) return;
            hide(true);
        },
        true,
    );

    document.addEventListener(
        "pointerdown",
        function (event) {
            const target = event.target as HTMLElement | null;
            if (portal && target && portal.contains(target)) return;
            const trigger = findTooltipTrigger(target);
            if (event.pointerType === "touch" && trigger && trigger.hasAttribute("data-sow-tooltip-tap")) {
                event.preventDefault();
                event.stopPropagation();
                if (anchorEl === trigger && portal && !portal.hidden) {
                    hide(true);
                } else {
                    show(trigger);
                }
                return;
            }
            hide(true);
        },
        true,
    );

    document.addEventListener("keydown", function (event) {
        if (event.key === "Escape") {
            hide(true);
        }
    });

    window.addEventListener(
        "scroll",
        function () {
            if (anchorEl && portal && !portal.hidden) {
                if (!anchorEl.isConnected) {
                    hide(true);
                } else if (currentSpec) {
                    positionPortal(portal, anchorEl, currentSpec);
                }
            }
        },
        true,
    );

    window.addEventListener("resize", function () {
        if (anchorEl && portal && !portal.hidden && currentSpec) {
            positionPortal(portal, anchorEl, currentSpec);
        }
    });

    window.addEventListener("blur", function () {
        hide(true);
    });
}

export function currentAnchorElement(): HTMLElement | null {
    return anchorEl;
}

export function currentTooltipPlacement(): "top" | "bottom" | "left" | "right" {
    return currentPlacement;
}

export const SOWTooltip = {
    init,
    register,
    set,
    clear,
    show,
    hide,
};

if (typeof window !== "undefined") {
    window.SOWTooltip = SOWTooltip;
    init();
}
