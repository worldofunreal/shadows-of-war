import { UI_TARGETS } from "./rules.ts";
import type { CampaignAnchor, CampaignRecord } from "./types.ts";

export function resolveUiTarget(
    key: string,
    root: ParentNode,
    episodeId?: string,
    _facts?: CampaignRecord,
): HTMLElement | null {
    const selector = UI_TARGETS[key];
    if (!selector || !root) return null;
    if (key === "campaign_replay") {
        if (!episodeId || typeof root.querySelectorAll !== "function") return null;
        return (
            Array.from(root.querySelectorAll<HTMLElement>(selector)).find(
                (node) => node.dataset && node.dataset.episodeId === episodeId,
            ) || null
        );
    }
    return typeof root.querySelector === "function" ? root.querySelector<HTMLElement>(selector) : null;
}

export function resolveUiAnchor(element: HTMLElement, spotlightElement?: HTMLElement | null): CampaignAnchor {
    const rect = element.getBoundingClientRect();
    const x = Number.parseFloat(element.dataset.tutorialAnchorX ?? "");
    const y = Number.parseFloat(element.dataset.tutorialAnchorY ?? "");
    const focus =
        Number.isFinite(x) &&
        Number.isFinite(y) &&
        element.querySelector &&
        element.querySelector(".sow-hud__map-action-title");
    const focusRect = focus && focus.getClientRects().length ? focus.getBoundingClientRect() : rect;
    const anchor: CampaignAnchor = {
        x: focus ? rect.left + (rect.width * x) / 100 : rect.left + rect.width / 2,
        y: focus ? rect.top + (rect.height * y) / 100 : rect.top + rect.height / 2,
        width: focusRect.width,
        height: focusRect.height,
    };
    if (spotlightElement) {
        const spotlight = spotlightElement.getBoundingClientRect();
        anchor.spotlightX = spotlight.left + spotlight.width / 2;
        anchor.spotlightY = spotlight.top + spotlight.height / 2;
        anchor.spotlightWidth = spotlight.width;
        anchor.spotlightHeight = spotlight.height;
    }
    return anchor;
}

export function resolveUiRangeAnchor(element: HTMLInputElement, value: number): CampaignAnchor | null {
    if (!element || typeof element.getBoundingClientRect !== "function") return null;
    const rect = element.getBoundingClientRect();
    const min = Number(element.min),
        max = Number(element.max);
    if (!(max > min) || ![rect.left, rect.top, rect.width, rect.height, value].every(Number.isFinite))
        return null;
    const view = element.ownerDocument && element.ownerDocument.defaultView;
    const style = view && typeof view.getComputedStyle === "function" ? view.getComputedStyle(element) : null;
    const vertical = style
        ? String(style.writingMode || "").startsWith("vertical")
        : rect.height > rect.width;
    const direction = (style && style.direction) || element.dir || "ltr";
    const thumbSize =
        style && typeof style.getPropertyValue === "function"
            ? parseFloat(style.getPropertyValue("--sow-range-thumb-size")) || 0
            : 0;
    const ratio = Math.max(0, Math.min(1, (value - min) / (max - min)));
    if (vertical) {
        const usable = Math.max(0, rect.height - thumbSize);
        const fromTop = direction === "rtl" ? 1 - ratio : ratio;
        return { x: rect.left + rect.width / 2, y: rect.top + thumbSize / 2 + fromTop * usable };
    }
    const usable = Math.max(0, rect.width - thumbSize);
    const fromLeft = direction === "rtl" ? 1 - ratio : ratio;
    return { x: rect.left + thumbSize / 2 + fromLeft * usable, y: rect.top + rect.height / 2 };
}

export function resolveUiRangeGuideAnchor(
    element: HTMLInputElement,
    value: number,
    targetValue: number,
): CampaignAnchor | null {
    const start = resolveUiRangeAnchor(element, value),
        end = resolveUiRangeAnchor(element, targetValue);
    if (!start || !end) return null;
    const frame = resolveUiAnchor(element);
    return {
        x: start.x,
        y: start.y,
        toX: end.x,
        toY: end.y,
        width: frame.width,
        height: frame.height,
        spotlightX: frame.x,
        spotlightY: frame.y,
        spotlightWidth: frame.width,
        spotlightHeight: frame.height,
    };
}
