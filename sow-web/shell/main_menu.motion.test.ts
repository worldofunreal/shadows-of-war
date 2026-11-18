import { describe, expect, it } from "bun:test";
import { computeScreenTransitionVector } from "./main_menu.motion.ts";

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
