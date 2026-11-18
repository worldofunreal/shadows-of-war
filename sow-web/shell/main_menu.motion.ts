export interface ScreenTransitionVector {
    kind: "rail-vertical" | "dock-horizontal" | "drill-in" | "drill-out" | "lateral";
    enterX: string;
    enterY: string;
    enterScale: string;
    leaveX: string;
    leaveY: string;
    leaveScale: string;
}

const SCREEN_NAV_GROUP: Record<string, string> = {
    home: "battle",
    campaign: "battle",
    browser: "battle",
    create: "battle",
    queue: "battle",
    heroes: "heroes",
    profile: "profile",
    store: "store",
};

const SCREEN_DEPTH: Record<string, number> = {
    home: 0,
    heroes: 0,
    profile: 0,
    store: 0,
    campaign: 1,
    browser: 1,
    create: 1,
    queue: 1,
};

const DESKTOP_RAIL_SLOT: Record<string, number> = {
    battle: 0,
    heroes: 1,
    profile: 2,
    store: 3,
};

const MOBILE_DOCK_SLOT: Record<string, number> = {
    store: 0,
    heroes: 1,
    battle: 2,
    profile: 3,
};

export function computeScreenTransitionVector(
    fromPanel: string | null | undefined,
    toPanel: string | null | undefined,
    isDesktopRail: boolean,
    isRtl = false,
): ScreenTransitionVector {
    const fromKey = fromPanel || "home";
    const toKey = toPanel || "home";
    const fromNav = SCREEN_NAV_GROUP[fromKey] || "battle";
    const toNav = SCREEN_NAV_GROUP[toKey] || "battle";

    if (fromNav === toNav) {
        const fromDepth = SCREEN_DEPTH[fromKey] ?? 0;
        const toDepth = SCREEN_DEPTH[toKey] ?? 0;
        if (toDepth > fromDepth) {
            return {
                kind: "drill-in",
                enterX: "0px",
                enterY: "18px",
                enterScale: "0.984",
                leaveX: "0px",
                leaveY: "-10px",
                leaveScale: "0.974",
            };
        }
        if (toDepth < fromDepth) {
            return {
                kind: "drill-out",
                enterX: "0px",
                enterY: "-12px",
                enterScale: "0.984",
                leaveX: "0px",
                leaveY: "14px",
                leaveScale: "0.982",
            };
        }
        return {
            kind: "lateral",
            enterX: "0px",
            enterY: "14px",
            enterScale: "0.988",
            leaveX: "0px",
            leaveY: "-10px",
            leaveScale: "0.985",
        };
    }

    if (isDesktopRail) {
        const fromSlot = DESKTOP_RAIL_SLOT[fromNav] ?? 0;
        const toSlot = DESKTOP_RAIL_SLOT[toNav] ?? 0;
        const dir = toSlot >= fromSlot ? 1 : -1;
        return {
            kind: "rail-vertical",
            enterX: "0px",
            enterY: dir * 24 + "px",
            enterScale: "0.988",
            leaveX: "0px",
            leaveY: dir * -18 + "px",
            leaveScale: "0.985",
        };
    }

    const fromSlot = MOBILE_DOCK_SLOT[fromNav] ?? 2;
    const toSlot = MOBILE_DOCK_SLOT[toNav] ?? 2;
    const rtlSign = isRtl ? -1 : 1;
    const dir = (toSlot >= fromSlot ? 1 : -1) * rtlSign;
    return {
        kind: "dock-horizontal",
        enterX: dir * 26 + "px",
        enterY: "6px",
        enterScale: "0.99",
        leaveX: dir * -20 + "px",
        leaveY: "0px",
        leaveScale: "0.986",
    };
}

export const sowScreenMotion = (function () {
    function reducedMotion() {
        return (
            typeof window.matchMedia === "function" &&
            window.matchMedia("(prefers-reduced-motion: reduce)").matches
        );
    }

    function desktopRailLayout() {
        return (
            typeof window.matchMedia === "function" &&
            window.matchMedia("(min-width: 681px) and (orientation: landscape)").matches
        );
    }

    function isRtlDocument() {
        return typeof document !== "undefined" && document.documentElement?.dir === "rtl";
    }

    function setInactive(panel: Element) {
        panel.classList.remove("is-active", "sow-screen-panel--entering");
        panel.classList.add("sow-screen-panel--leaving");
        panel.setAttribute("aria-hidden", "true");
        if ("inert" in panel) panel.inert = true;
    }

    function setActive(panel: Element) {
        panel.classList.add("is-active");
        panel.classList.remove("sow-screen-panel--leaving");
        panel.removeAttribute("aria-hidden");
        if ("inert" in panel) panel.inert = false;
    }

    function finish(event: AnimationEvent) {
        if (event.target === event.currentTarget && event.animationName === "sow-stage-light-sweep") {
            (event.currentTarget as HTMLElement).classList.remove("sow-screen-stage--sweeping");
            return;
        }
        const panel = event.target as Element | null;
        if (!panel || panel.parentNode !== event.currentTarget || !panel.matches("[data-screen-panel]"))
            return;
        if (event.animationName === "sow-screen-panel-leave") panel.remove();
        if (event.animationName === "sow-screen-panel-enter") {
            panel.classList.remove("sow-screen-panel--entering");
            (event.currentTarget as HTMLElement).classList.remove("sow-screen-stage--sweeping");
        }
    }

    function bind(stage: HTMLElement) {
        if (stage.dataset.motionBound) return;
        stage.dataset.motionBound = "true";
        stage.addEventListener("animationend", finish);
        stage.addEventListener("animationcancel", finish);
    }

    function applyVector(stage: HTMLElement, vector: ScreenTransitionVector) {
        stage.dataset.motionKind = vector.kind;
        stage.style.setProperty("--sow-screen-enter-x", vector.enterX);
        stage.style.setProperty("--sow-screen-enter-y", vector.enterY);
        stage.style.setProperty("--sow-screen-enter-scale", vector.enterScale);
        stage.style.setProperty("--sow-screen-leave-x", vector.leaveX);
        stage.style.setProperty("--sow-screen-leave-y", vector.leaveY);
        stage.style.setProperty("--sow-screen-leave-scale", vector.leaveScale);
    }

    function show(stage: HTMLElement, next: HTMLElement, disableMotion: boolean) {
        bind(stage);
        const current = stage.querySelector("[data-screen-panel].is-active") || stage.lastElementChild;
        const panels = stage.querySelectorAll("[data-screen-panel]");
        for (let i = 0; i < panels.length; i++) {
            if (panels[i] !== current) panels[i].remove();
        }
        const fromPanel = current ? current.getAttribute("data-screen-panel") : null;
        const toPanel = next.getAttribute("data-screen-panel");
        const vector = computeScreenTransitionVector(
            fromPanel,
            toPanel,
            desktopRailLayout(),
            isRtlDocument(),
        );
        applyVector(stage, vector);
        if (current) setInactive(current);
        setActive(next);
        if (disableMotion || reducedMotion()) {
            stage.classList.remove("sow-screen-stage--sweeping");
            if (current) current.remove();
            stage.appendChild(next);
            return;
        }
        stage.classList.remove("sow-screen-stage--sweeping");
        void stage.offsetWidth;
        stage.classList.add("sow-screen-stage--sweeping");
        next.classList.add("sow-screen-panel--entering");
        stage.appendChild(next);
    }

    return { show: show };
})();
