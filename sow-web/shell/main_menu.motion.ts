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
        if (
            (fromKey === "browser" && toKey === "create") ||
            (fromKey === "create" && toKey === "browser")
        ) {
            const tabDir = (toKey === "create" ? 1 : -1) * (isRtl ? -1 : 1);
            return {
                kind: "lateral",
                enterX: `${tabDir * 22}px`,
                enterY: "0px",
                enterScale: "0.992",
                leaveX: `${-tabDir * 16}px`,
                leaveY: "0px",
                leaveScale: "0.99",
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

export type GpuPerfTier = "low" | "high";

export interface GpuTierEnvironment {
    coarsePointer?: boolean;
    deviceMemory?: number;
    hardwareConcurrency?: number;
}

/**
 * Classifies devices with coarse touch pointers or constrained memory/CPU cores
 * as low-end tile GPUs so CSS and WAAPI can skip multi-pass blur and heavy masks.
 */
export function detectLowEndGpuTier(env?: GpuTierEnvironment): GpuPerfTier {
    const coarsePointer =
        env?.coarsePointer !== undefined
            ? env.coarsePointer
            : typeof window !== "undefined" &&
              typeof window.matchMedia === "function" &&
              window.matchMedia("(hover: none) and (pointer: coarse)").matches;
    if (coarsePointer) return "low";

    const nav =
        typeof navigator !== "undefined"
            ? (navigator as Navigator & { deviceMemory?: number })
            : undefined;
    const deviceMemory = env?.deviceMemory !== undefined ? env.deviceMemory : nav?.deviceMemory;
    if (typeof deviceMemory === "number" && Number.isFinite(deviceMemory) && deviceMemory > 0 && deviceMemory <= 4) {
        return "low";
    }

    const hardwareConcurrency =
        env?.hardwareConcurrency !== undefined ? env.hardwareConcurrency : nav?.hardwareConcurrency;
    if (
        typeof hardwareConcurrency === "number" &&
        Number.isFinite(hardwareConcurrency) &&
        hardwareConcurrency > 0 &&
        hardwareConcurrency <= 4
    ) {
        return "low";
    }

    return "high";
}

let gpuTierBound = false;

export function applyGpuPerfTier(): GpuPerfTier {
    const tier = detectLowEndGpuTier();
    if (typeof document !== "undefined" && document.documentElement) {
        document.documentElement.dataset.sowGpuTier = tier;
    }
    if (!gpuTierBound && typeof window !== "undefined" && typeof window.matchMedia === "function") {
        gpuTierBound = true;
        const mq = window.matchMedia("(hover: none) and (pointer: coarse)");
        const onChange = function () {
            if (typeof document !== "undefined" && document.documentElement) {
                document.documentElement.dataset.sowGpuTier = detectLowEndGpuTier();
            }
        };
        if (typeof mq.addEventListener === "function") mq.addEventListener("change", onChange);
    }
    return tier;
}

export function isLowEndGpuTier(): boolean {
    if (typeof document !== "undefined" && document.documentElement?.dataset?.sowGpuTier) {
        return document.documentElement.dataset.sowGpuTier === "low";
    }
    return detectLowEndGpuTier() === "low";
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
        const active = typeof document !== "undefined" ? document.activeElement : null;
        if (active instanceof HTMLElement && panel.contains(active)) active.blur();
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
        if (typeof stage.getAnimations === "function") {
            stage.getAnimations().forEach(function (anim) {
                if ((anim as CSSAnimation).animationName === "sow-stage-light-sweep") {
                    try {
                        anim.cancel();
                    } catch {}
                }
            });
        }
        next.classList.add("sow-screen-panel--entering");
        stage.appendChild(next);
        if (!isLowEndGpuTier()) {
            if (typeof window.requestAnimationFrame === "function") {
                window.requestAnimationFrame(function () {
                    if (next.isConnected) stage.classList.add("sow-screen-stage--sweeping");
                });
            } else {
                stage.classList.add("sow-screen-stage--sweeping");
            }
        }
    }

    return { show: show };
})();

export type RewardStageKind = "xp" | "crowns" | "gems" | "laurels" | "leader" | "level" | "achievement";

export interface RewardChoreographyStageInput {
    kind: RewardStageKind;
    amount: number;
}

export interface RewardChoreographyOptions {
    xpFillPct?: number;
    levelsGained?: number;
    hasBanner?: boolean;
}

export interface PlannedRewardStageTiming {
    index: number;
    kind: RewardStageKind;
    revealDelayMs: number;
    revealDurationMs: number;
    countStartMs: number;
    countDurationMs: number;
    launchAtMs: number;
    flyDurationMs: number;
    landAtMs: number;
    applyDurationMs: number;
    settleAtMs: number;
}

export interface RewardChoreographyPlan {
    stages: PlannedRewardStageTiming[];
    curtainDurationMs: number;
    revealEndMs: number;
    launchPhaseStartMs: number;
    exitAtMs: number;
    exitDurationMs: number;
    totalDurationMs: number;
}

export type RewardPlaybackMode = "normal" | "accelerating" | "immediate";

export interface AcceleratedChoreographyResult {
    plan: RewardChoreographyPlan;
    speed: number;
    exitSpeed: number;
}

export interface FlyArcPoint {
    offset: number;
    x: number;
    y: number;
    scale: number;
    rotateDeg: number;
    opacity: number;
}

function clampNumber(min: number, value: number, max: number): number {
    return Math.max(min, Math.min(max, value));
}

function easeInOutCubic(t: number): number {
    return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
}

/**
 * Builds a staggered, overlapping choreography timeline for post-match or mission rewards.
 * Preserves order: server-confirmed rewards -> unlocked leaders -> each gained level (ascending).
 */
export function planRewardChoreography(
    inputs: readonly RewardChoreographyStageInput[],
    options: RewardChoreographyOptions = {},
): RewardChoreographyPlan {
    const curtainDurationMs = 160;
    const count = inputs.length;
    if (!count) {
        return {
            stages: [],
            curtainDurationMs,
            revealEndMs: 0,
            launchPhaseStartMs: 0,
            exitAtMs: 0,
            exitDurationMs: 160,
            totalDurationMs: 0,
        };
    }

    const revealStaggerMs = count <= 1 ? 0 : Math.round(clampNumber(36, 210 / count, 54));
    const revealDurationMs = 350;
    const xpFillPct = clampNumber(0, Number(options.xpFillPct) || 0, 100);
    const levelsGained = Math.max(0, Math.floor(Number(options.levelsGained) || 0));
    const hasBanner = Boolean(options.hasBanner);
    let lastReadyMs = 0;
    const stages: PlannedRewardStageTiming[] = inputs.map((input, index) => {
        const revealDelayMs = 40 + index * revealStaggerMs;
        const isNumeric =
            input.kind === "xp" ||
            input.kind === "crowns" ||
            input.kind === "gems" ||
            input.kind === "laurels";
        const countStartMs = revealDelayMs + 80;
        const countDurationMs = isNumeric ? (count <= 2 ? 280 : 320) : 0;
        const readyMs = Math.max(revealDelayMs + revealDurationMs, countStartMs + countDurationMs);
        if (readyMs > lastReadyMs) lastReadyMs = readyMs;
        return {
            index,
            kind: input.kind,
            revealDelayMs,
            revealDurationMs,
            countStartMs,
            countDurationMs,
            launchAtMs: 0,
            flyDurationMs: 0,
            landAtMs: 0,
            applyDurationMs: 0,
            settleAtMs: 0,
        };
    });

    if (hasBanner) {
        lastReadyMs = Math.max(lastReadyMs, 520);
    }
    // Keep the complete set readable before the first card leaves.
    const revealHoldMs = 1000;
    const revealEndMs = lastReadyMs + revealHoldMs;
    const launchPhaseStartMs = revealEndMs;

    const rewardIndices: number[] = [];
    const leaderIndices: number[] = [];
    const levelIndices: number[] = [];
    stages.forEach((stage, index) => {
        if (stage.kind === "leader") leaderIndices.push(index);
        else if (stage.kind === "level") levelIndices.push(index);
        else rewardIndices.push(index);
    });

    const rewardStaggerMs = rewardIndices.length <= 1 ? 0 : 240;
    const rewardFlyMs = 370;

    let wave1LastLaunch = launchPhaseStartMs;
    let wave1LastLand = launchPhaseStartMs;
    let xpSettleAtMs = launchPhaseStartMs;
    let lastSettleAtMs = launchPhaseStartMs;

    rewardIndices.forEach((stageIdx, pos) => {
        const stage = stages[stageIdx];
        const launchAtMs = launchPhaseStartMs + pos * rewardStaggerMs;
        const landAtMs = launchAtMs + rewardFlyMs;
        let applyDurationMs = 0;
        if (stage.kind === "crowns" || stage.kind === "gems") {
            applyDurationMs = 260;
        } else if (stage.kind === "xp") {
            applyDurationMs = Math.round(
                clampNumber(340, 320 + (xpFillPct / 100) * 280 + (levelsGained > 0 ? 120 : 0), 960),
            );
        }
        const settleAtMs = landAtMs + applyDurationMs;
        stage.launchAtMs = launchAtMs;
        stage.flyDurationMs = rewardFlyMs;
        stage.landAtMs = landAtMs;
        stage.applyDurationMs = applyDurationMs;
        stage.settleAtMs = settleAtMs;

        wave1LastLaunch = Math.max(wave1LastLaunch, launchAtMs);
        wave1LastLand = Math.max(wave1LastLand, landAtMs);
        if (stage.kind === "xp") xpSettleAtMs = settleAtMs;
        lastSettleAtMs = Math.max(lastSettleAtMs, settleAtMs);
    });

    let wave2LastLaunch = wave1LastLaunch;
    if (leaderIndices.length) {
        const leaderStart = rewardIndices.length > 0 ? wave1LastLaunch + 64 : launchPhaseStartMs;
        const leaderStaggerMs = 250;
        const leaderFlyMs = 390;
        leaderIndices.forEach((stageIdx, pos) => {
            const stage = stages[stageIdx];
            const launchAtMs = leaderStart + pos * leaderStaggerMs;
            const landAtMs = launchAtMs + leaderFlyMs;
            const applyDurationMs = 120;
            const settleAtMs = landAtMs + applyDurationMs;
            stage.launchAtMs = launchAtMs;
            stage.flyDurationMs = leaderFlyMs;
            stage.landAtMs = landAtMs;
            stage.applyDurationMs = applyDurationMs;
            stage.settleAtMs = settleAtMs;

            wave2LastLaunch = Math.max(wave2LastLaunch, launchAtMs);
            lastSettleAtMs = Math.max(lastSettleAtMs, settleAtMs);
        });
    }

    if (levelIndices.length) {
        const levelAnchor = Math.max(
            launchPhaseStartMs,
            wave1LastLand,
            xpSettleAtMs,
            leaderIndices.length > 0 ? wave2LastLaunch + 64 : launchPhaseStartMs,
        );
        const levelStaggerMs = 250;
        const levelFlyMs = 300;
        levelIndices.forEach((stageIdx, pos) => {
            const stage = stages[stageIdx];
            const isFinalLevel = pos === levelIndices.length - 1;
            const launchAtMs = levelAnchor + pos * levelStaggerMs;
            const landAtMs = launchAtMs + levelFlyMs;
            const applyDurationMs = isFinalLevel ? 340 : 210;
            const settleAtMs = landAtMs + applyDurationMs;
            stage.launchAtMs = launchAtMs;
            stage.flyDurationMs = levelFlyMs;
            stage.landAtMs = landAtMs;
            stage.applyDurationMs = applyDurationMs;
            stage.settleAtMs = settleAtMs;

            lastSettleAtMs = Math.max(lastSettleAtMs, settleAtMs);
        });
    }

    const exitAtMs = lastSettleAtMs + 32;
    const exitDurationMs = 180;
    return {
        stages,
        curtainDurationMs,
        revealEndMs,
        launchPhaseStartMs,
        exitAtMs,
        exitDurationMs,
        totalDurationMs: exitAtMs + exitDurationMs,
    };
}

/**
 * Compresses remaining idle gaps and future stage launches on first skip so the
 * choreography accelerates smoothly into early completion instead of snapping abruptly.
 */
export function accelerateChoreographyPlan(
    plan: RewardChoreographyPlan,
    virtualNowMs: number,
    targetRealMs = 220,
): AcceleratedChoreographyResult {
    const vNow = Math.max(0, virtualNowMs);
    if (!plan.stages.length || vNow >= plan.exitAtMs) {
        return {
            plan,
            speed: 5.5,
            exitSpeed: 1.5,
        };
    }

    const nextStages = plan.stages.map((s) => Object.assign({}, s));
    const rewardGap = 36;
    const leaderGap = 48;
    const levelGap = 56;
    const firstLaunch = Math.max(vNow, Math.min(plan.launchPhaseStartMs, vNow + 36));

    let rewardCursor = firstLaunch;
    let hasWave1 = false;
    let wave1MaxLaunch = vNow;
    let wave1MaxLand = vNow;
    let xpSettleAt = vNow;
    let maxSettleAt = vNow;

    for (const stage of nextStages) {
        if (stage.kind === "leader" || stage.kind === "level") continue;
        hasWave1 = true;
        if (stage.launchAtMs > vNow) {
            stage.launchAtMs = Math.min(stage.launchAtMs, rewardCursor);
            stage.landAtMs = stage.launchAtMs + stage.flyDurationMs;
            rewardCursor = stage.launchAtMs + rewardGap;
        }
        if (stage.applyDurationMs > 0) {
            const remainingApply = Math.max(0, stage.settleAtMs - Math.max(vNow, stage.landAtMs));
            const capApply = stage.kind === "xp" ? 280 : 180;
            if (remainingApply > capApply) {
                const startApply = Math.max(vNow, stage.landAtMs);
                stage.settleAtMs = startApply + capApply;
                stage.applyDurationMs = Math.max(1, stage.settleAtMs - stage.landAtMs);
            } else {
                stage.settleAtMs = stage.landAtMs + stage.applyDurationMs;
            }
        } else {
            stage.settleAtMs = stage.landAtMs;
        }
        wave1MaxLaunch = Math.max(wave1MaxLaunch, stage.launchAtMs);
        wave1MaxLand = Math.max(wave1MaxLand, stage.landAtMs);
        if (stage.kind === "xp") xpSettleAt = Math.max(xpSettleAt, stage.settleAtMs);
        maxSettleAt = Math.max(maxSettleAt, stage.settleAtMs);
    }

    let leaderCursor = hasWave1 ? Math.max(firstLaunch, wave1MaxLaunch + 48) : firstLaunch;
    let hasWave2 = false;
    let wave2MaxLaunch = wave1MaxLaunch;
    for (const stage of nextStages) {
        if (stage.kind !== "leader") continue;
        hasWave2 = true;
        if (stage.launchAtMs > vNow) {
            stage.launchAtMs = Math.min(stage.launchAtMs, leaderCursor);
            stage.landAtMs = stage.launchAtMs + stage.flyDurationMs;
            stage.applyDurationMs = Math.min(stage.applyDurationMs, 70);
            stage.settleAtMs = stage.landAtMs + stage.applyDurationMs;
            leaderCursor = stage.launchAtMs + leaderGap;
        } else {
            stage.settleAtMs = Math.min(stage.settleAtMs, Math.max(vNow, stage.landAtMs) + 70);
        }
        wave2MaxLaunch = Math.max(wave2MaxLaunch, stage.launchAtMs);
        maxSettleAt = Math.max(maxSettleAt, stage.settleAtMs);
    }

    let levelCursor = Math.max(
        firstLaunch,
        wave1MaxLand,
        xpSettleAt,
        hasWave2 ? wave2MaxLaunch + 48 : firstLaunch,
    );
    for (const stage of nextStages) {
        if (stage.kind !== "level") continue;
        if (stage.launchAtMs > vNow) {
            stage.launchAtMs = Math.min(stage.launchAtMs, levelCursor);
            stage.landAtMs = stage.launchAtMs + stage.flyDurationMs;
            stage.applyDurationMs = Math.min(stage.applyDurationMs, 180);
            stage.settleAtMs = stage.landAtMs + stage.applyDurationMs;
            levelCursor = stage.launchAtMs + levelGap;
        } else {
            const remainingApply = Math.max(0, stage.settleAtMs - Math.max(vNow, stage.landAtMs));
            if (remainingApply > 180) {
                stage.settleAtMs = Math.max(vNow, stage.landAtMs) + 180;
                stage.applyDurationMs = Math.max(1, stage.settleAtMs - stage.landAtMs);
            }
        }
        maxSettleAt = Math.max(maxSettleAt, stage.settleAtMs);
    }

    const exitAtMs = Math.min(plan.exitAtMs, Math.max(vNow, maxSettleAt + 36));
    const remainingVirtual = Math.max(1, exitAtMs - vNow);
    const speed = clampNumber(5.5, remainingVirtual / Math.max(80, targetRealMs), 10);
    // Keep the exit curtain fade readable (~115ms real time) even under high cascade speed.
    const exitSpeed = 1.55;
    const nextPlan: RewardChoreographyPlan = {
        stages: nextStages,
        curtainDurationMs: plan.curtainDurationMs,
        revealEndMs: Math.min(plan.revealEndMs, firstLaunch),
        launchPhaseStartMs: firstLaunch,
        exitAtMs,
        exitDurationMs: plan.exitDurationMs,
        totalDurationMs: exitAtMs + plan.exitDurationMs,
    };
    return { plan: nextPlan, speed, exitSpeed };
}

/**
 * Samples an analytically eased quadratic Bezier arc so WAAPI can run with linear
 * segment interpolation (zero per-keyframe stutter and smooth playbackRate scaling).
 */
export function sampleFlyArcPoints(dx: number, dy: number, steps = 16): FlyArcPoint[] {
    const safeSteps = Math.max(4, Math.floor(steps));
    const bend = clampNumber(56, Math.hypot(dx, dy) * 0.18, 140);
    const controlX = dx * 0.5 + (dx < 0 ? -bend * 0.55 : bend * 0.55);
    const controlY = dy * 0.5 - bend;
    const tiltDir = dx < 0 ? -1 : 1;

    const points: FlyArcPoint[] = [
        { offset: 0, x: 0, y: 0, scale: 1, rotateDeg: 0, opacity: 1 },
        { offset: 0.08, x: 0, y: 5, scale: 1.055, rotateDeg: 0, opacity: 1 },
    ];

    for (let i = 1; i <= safeSteps; i++) {
        const r = i / safeSteps;
        const t = easeInOutCubic(r);
        const inv = 1 - t;
        const x = 2 * inv * t * controlX + t * t * dx;
        const y = 2 * inv * t * controlY + t * t * dy + 5 * (1 - t);
        const scale = 1.055 - 0.815 * t;
        const rotateDeg = Math.sin(Math.PI * t) * 7 * tiltDir;
        const opacity = t > 0.82 ? 1 - 0.42 * ((t - 0.82) / 0.18) : 1;
        points.push({
            offset: 0.08 + 0.92 * r,
            x,
            y,
            scale,
            rotateDeg,
            opacity,
        });
    }
    return points;
}

export function flyArcPointsToKeyframes(points: readonly FlyArcPoint[]): Keyframe[] {
    return points.map((p) => ({
        offset: p.offset,
        transform:
            "translate3d(" +
            p.x.toFixed(2) +
            "px," +
            p.y.toFixed(2) +
            "px,0) scale(" +
            p.scale.toFixed(4) +
            ") rotate(" +
            p.rotateDeg.toFixed(2) +
            "deg)",
        opacity: Number(p.opacity.toFixed(4)),
    }));
}

/**
 * Returns staggered launch offsets (ms) for multi-token store bundle gem bursts.
 */
export function planStoreBurstStagger(count: number, staggerMs = 55): number[] {
    const safeCount = Math.max(0, Math.floor(count));
    const safeStagger = Math.max(0, staggerMs);
    const delays: number[] = [];
    for (let i = 0; i < safeCount; i++) {
        delays.push(i * safeStagger);
    }
    return delays;
}

/**
 * Computes staggered easeOutCubic roll-up values for HUD endgame stat rows.
 */
export function computeEndgameStatRollups(
    elapsedMs: number,
    targets: readonly number[],
    baseDelayMs = 200,
    staggerMs = 45,
    durationMs = 280,
): number[] {
    const safeDuration = Math.max(1, durationMs);
    return targets.map(function (rawTarget, index) {
        const target = Math.max(0, Math.round(Number(rawTarget) || 0));
        if (target === 0) return 0;
        const start = baseDelayMs + index * staggerMs;
        if (elapsedMs <= start) return 0;
        const t = Math.min(1, (elapsedMs - start) / safeDuration);
        const eased = 1 - Math.pow(1 - t, 3);
        return Math.round(target * eased);
    });
}
