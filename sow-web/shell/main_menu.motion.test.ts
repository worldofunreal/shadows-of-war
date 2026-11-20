import { describe, expect, it } from "bun:test";
import {
    accelerateChoreographyPlan,
    computeEndgameStatRollups,
    computeScreenTransitionVector,
    detectLowEndGpuTier,
    flyArcPointsToKeyframes,
    planRewardChoreography,
    planStoreBurstStagger,
    sampleFlyArcPoints,
} from "./main_menu.motion.ts";

describe("computeScreenTransitionVector", () => {
    it("computes vertical rail transitions on desktop landscape", () => {
        // battle (slot 0) -> heroes (slot 1): moving down the rail
        const down = computeScreenTransitionVector("home", "heroes", true, false);
        expect(down.kind).toBe("rail-vertical");
        expect(down.enterX).toBe("0px");
        expect(down.enterY).toBe("24px");
        expect(down.leaveY).toBe("-18px");

        // store (slot 3) -> battle (slot 0): moving up the rail
        const up = computeScreenTransitionVector("store", "home", true, false);
        expect(up.kind).toBe("rail-vertical");
        expect(up.enterX).toBe("0px");
        expect(up.enterY).toBe("-24px");
        expect(up.leaveY).toBe("18px");
    });

    it("computes horizontal dock transitions on mobile/portrait (LTR & RTL)", () => {
        // store (slot 0) -> battle (slot 2): moving right on mobile dock
        const rightLtr = computeScreenTransitionVector("store", "home", false, false);
        expect(rightLtr.kind).toBe("dock-horizontal");
        expect(rightLtr.enterX).toBe("26px");
        expect(rightLtr.leaveX).toBe("-20px");

        // profile (slot 3) -> heroes (slot 1): moving left on mobile dock
        const leftLtr = computeScreenTransitionVector("profile", "heroes", false, false);
        expect(leftLtr.kind).toBe("dock-horizontal");
        expect(leftLtr.enterX).toBe("-26px");
        expect(leftLtr.leaveX).toBe("20px");

        // RTL mirrors horizontal direction
        const rightRtl = computeScreenTransitionVector("store", "home", false, true);
        expect(rightRtl.kind).toBe("dock-horizontal");
        expect(rightRtl.enterX).toBe("-26px");
        expect(rightRtl.leaveX).toBe("20px");
    });

    it("computes depth drill-in and drill-out within the battle screen group", () => {
        for (const sub of ["campaign", "browser", "create", "queue"]) {
            const drillIn = computeScreenTransitionVector("home", sub, true, false);
            expect(drillIn.kind).toBe("drill-in");
            expect(drillIn.enterScale).toBe("0.984");
            expect(drillIn.leaveScale).toBe("0.974");

            const drillOut = computeScreenTransitionVector(sub, "home", true, false);
            expect(drillOut.kind).toBe("drill-out");
            expect(drillOut.enterY).toBe("-12px");
            expect(drillOut.leaveY).toBe("14px");
        }
    });
});

describe("planRewardChoreography", () => {
    it("handles empty reward stages cleanly", () => {
        const plan = planRewardChoreography([]);
        expect(plan.stages).toEqual([]);
        expect(plan.totalDurationMs).toBe(0);
    });

    it("keeps single reward timing snappy without a fixed 1450ms reveal plateau", () => {
        const plan = planRewardChoreography([{ kind: "xp", amount: 120 }], { xpFillPct: 40 });
        expect(plan.revealEndMs).toBeLessThan(650);
        expect(plan.stages[0].countStartMs + plan.stages[0].countDurationMs).toBeLessThanOrEqual(
            plan.launchPhaseStartMs,
        );
        expect(plan.totalDurationMs).toBeLessThan(1650);
    });

    it("overlaps multi-reward launches while keeping rewards -> leaders -> levels order", () => {
        const plan = planRewardChoreography(
            [
                { kind: "xp", amount: 520 },
                { kind: "crowns", amount: 180 },
                { kind: "gems", amount: 25 },
                { kind: "laurels", amount: 3 },
                { kind: "leader", amount: 0 },
                { kind: "level", amount: 4 },
                { kind: "level", amount: 5 },
            ],
            { xpFillPct: 85, levelsGained: 2 },
        );

        // Every numeric card completes its count-up before launchPhaseStartMs (even with 7 cards).
        for (const s of plan.stages) {
            if (s.countDurationMs > 0) {
                expect(s.countStartMs + s.countDurationMs).toBeLessThanOrEqual(plan.launchPhaseStartMs);
            }
        }

        const xp = plan.stages[0];
        const crowns = plan.stages[1];
        const gems = plan.stages[2];
        const laurels = plan.stages[3];
        const leader = plan.stages[4];
        const lvl4 = plan.stages[5];
        const lvl5 = plan.stages[6];

        // Wave 1 overlaps: crowns launches while xp is still flying, laurels has zero invisible apply wait.
        expect(crowns.launchAtMs).toBeGreaterThan(xp.launchAtMs);
        expect(crowns.launchAtMs).toBeLessThan(xp.landAtMs);
        expect(gems.launchAtMs).toBeLessThan(crowns.landAtMs);
        expect(laurels.applyDurationMs).toBe(0);

        // Wave 2 leader spotlights briefly after Wave 1 launches.
        expect(leader.launchAtMs).toBeGreaterThan(laurels.launchAtMs);

        // Wave 3 gained levels fire strictly after XP settle and Wave 1 arrivals, in ascending order.
        expect(lvl4.launchAtMs).toBeGreaterThanOrEqual(xp.settleAtMs);
        expect(lvl4.launchAtMs).toBeGreaterThanOrEqual(laurels.landAtMs);
        expect(lvl5.launchAtMs).toBeGreaterThan(lvl4.launchAtMs);
        expect(lvl5.applyDurationMs).toBeGreaterThan(lvl4.applyDurationMs);

        // Whole 7-card mega sequence completes in ~2.6-3.1s instead of 10+s.
        expect(plan.totalDurationMs).toBeLessThan(3200);
    });
});

describe("accelerateChoreographyPlan", () => {
    it("compresses remaining stagger and scales speed when skipped during reveal", () => {
        const plan = planRewardChoreography(
            [
                { kind: "xp", amount: 400 },
                { kind: "crowns", amount: 150 },
                { kind: "gems", amount: 15 },
                { kind: "leader", amount: 0 },
                { kind: "level", amount: 2 },
            ],
            { xpFillPct: 75, levelsGained: 1 },
        );

        const accel = accelerateChoreographyPlan(plan, 120);
        expect(accel.speed).toBeGreaterThanOrEqual(5.5);
        expect(accel.exitSpeed).toBeLessThan(accel.speed);
        const realMsToExit = (accel.plan.exitAtMs - 120) / accel.speed;
        expect(realMsToExit).toBeLessThanOrEqual(240);

        // Order invariant remains intact under acceleration.
        const [xp, crowns, gems, leader, lvl] = accel.plan.stages;
        expect(xp.launchAtMs).toBeGreaterThanOrEqual(120);
        expect(crowns.launchAtMs).toBeGreaterThan(xp.launchAtMs);
        expect(gems.launchAtMs).toBeGreaterThan(crowns.launchAtMs);
        expect(leader.launchAtMs).toBeGreaterThan(gems.launchAtMs);
        expect(lvl.launchAtMs).toBeGreaterThanOrEqual(xp.settleAtMs);
    });

    it("does not rewind already-launched stages when skipped mid-cascade", () => {
        const plan = planRewardChoreography(
            [
                { kind: "xp", amount: 300 },
                { kind: "crowns", amount: 100 },
                { kind: "level", amount: 3 },
            ],
            { xpFillPct: 90, levelsGained: 1 },
        );
        const midCascadeNow = plan.stages[1].launchAtMs + 40;
        const accel = accelerateChoreographyPlan(plan, midCascadeNow);

        // Already-launched stages keep their past launch timestamps unchanged.
        expect(accel.plan.stages[0].launchAtMs).toBe(plan.stages[0].launchAtMs);
        expect(accel.plan.stages[1].launchAtMs).toBe(plan.stages[1].launchAtMs);
        // Unlaunched level stage pulls closer without preceding active XP settle.
        expect(accel.plan.stages[2].launchAtMs).toBeGreaterThanOrEqual(accel.plan.stages[0].settleAtMs);
        expect(accel.plan.exitAtMs).toBeLessThanOrEqual(plan.exitAtMs);
    });
});

describe("sampleFlyArcPoints", () => {
    it("produces monotonic offsets, anticipation wind-up, and exact target endpoint", () => {
        const points = sampleFlyArcPoints(320, -240, 16);
        expect(points.length).toBe(18);
        expect(points[0]).toMatchObject({ offset: 0, x: 0, y: 0, scale: 1, opacity: 1 });
        expect(points[1].offset).toBe(0.08);
        expect(points[1].scale).toBeGreaterThan(1);

        const last = points[points.length - 1];
        expect(last.offset).toBe(1);
        expect(last.x).toBeCloseTo(320, 4);
        expect(last.y).toBeCloseTo(-240, 4);

        for (let i = 1; i < points.length; i++) {
            expect(points[i].offset).toBeGreaterThan(points[i - 1].offset);
        }

        const keyframes = flyArcPointsToKeyframes(points);
        expect(keyframes.length).toBe(points.length);
        expect(String(keyframes[0].transform)).toContain("translate3d(0.00px,0.00px,0)");
    });
});

describe("detectLowEndGpuTier", () => {
    it("classifies coarse pointers and low-memory or low-core devices as low", () => {
        expect(detectLowEndGpuTier({ coarsePointer: true, deviceMemory: 8, hardwareConcurrency: 8 })).toBe("low");
        expect(detectLowEndGpuTier({ coarsePointer: false, deviceMemory: 4, hardwareConcurrency: 8 })).toBe("low");
        expect(detectLowEndGpuTier({ coarsePointer: false, deviceMemory: 8, hardwareConcurrency: 4 })).toBe("low");
        expect(detectLowEndGpuTier({ coarsePointer: false, deviceMemory: 8, hardwareConcurrency: 8 })).toBe("high");
    });
});

describe("planStoreBurstStagger & computeEndgameStatRollups", () => {
    it("staggers store burst tokens evenly", () => {
        expect(planStoreBurstStagger(3, 55)).toEqual([0, 55, 110]);
        expect(planStoreBurstStagger(0, 55)).toEqual([]);
    });

    it("rolls up endgame stats in staggered order and settles at final targets", () => {
        const targets = [12, 4, 1500, 2400, 800];
        expect(computeEndgameStatRollups(100, targets)).toEqual([0, 0, 0, 0, 0]);
        const mid = computeEndgameStatRollups(300, targets);
        expect(mid[0]).toBeGreaterThan(0);
        expect(mid[0]).toBeLessThan(12);
        expect(mid[4]).toBe(0);
        expect(computeEndgameStatRollups(1000, targets)).toEqual(targets);
    });
});
