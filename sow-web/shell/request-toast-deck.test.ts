import { describe, expect, test } from "bun:test";
import type { HudInboxRequest } from "./main_menu.state.ts";
import {
    MAX_REQUEST_TOAST_CARDS,
    REQUEST_TOAST_TTL_MS,
    createRequestToastDeckState,
    dismissAllRequestToasts,
    dismissFrontRequestToast,
    dismissRequestToastByKey,
    isValidInboxRequest,
    readRequestCountdown,
    reconcileRequestToastDeck,
    requestKey,
    syncRequestCountdowns,
    type RequestCountdownRecord,
} from "./request-toast-deck.ts";

function allianceReq(id: number, remainingSeconds = 20, isRenewal = false): HudInboxRequest {
    return {
        kind: "alliance",
        requester_id: id,
        name: `Player ${id}`,
        avatar: { kind: "fallback" },
        active: true,
        is_renewal: isRenewal,
        remaining_seconds: remainingSeconds,
        ttl_seconds: 20,
    };
}

function resourceReq(
    id: number,
    gold = 250,
    troops = 0,
    remainingSeconds = 30,
): HudInboxRequest {
    return {
        kind: "resources",
        requester_id: id,
        name: `Ally ${id}`,
        avatar: { kind: "fallback" },
        active: true,
        remaining_seconds: remainingSeconds,
        ttl_seconds: 30,
        gold,
        troops,
        pay_gold: 0,
        pay_troops: 0,
    };
}

describe("request-toast-deck", () => {
    test("groups duplicates and caps visible deck to 4 cards with overflow count", () => {
        const state = createRequestToastDeckState();
        const countdowns = new Map<string, RequestCountdownRecord>();
        const requests = [
            allianceReq(1),
            allianceReq(1),
            allianceReq(2),
            resourceReq(3, 500, 0),
            allianceReq(4),
            resourceReq(5, 0, 400),
            allianceReq(6),
        ];

        const now = 10_000;
        syncRequestCountdowns(requests, countdowns, now);
        const view = reconcileRequestToastDeck(state, requests, countdowns, now);

        expect(view.totalPending).toBe(6);
        expect(view.visible.length).toBe(MAX_REQUEST_TOAST_CARDS);
        expect(view.overflowCount).toBe(2);
    });

    test("promotes next card on front dismiss and keeps request available in inbox", () => {
        const state = createRequestToastDeckState();
        const countdowns = new Map<string, RequestCountdownRecord>();
        const requests = [allianceReq(10), resourceReq(20, 100, 0)];

        const now = 20_000;
        syncRequestCountdowns(requests, countdowns, now);
        let view = reconcileRequestToastDeck(state, requests, countdowns, now);
        expect(view.visible.map((v) => v.key)).toEqual(["alliance:10", "resources:20"]);

        expect(dismissFrontRequestToast(state)).toBe(true);
        view = reconcileRequestToastDeck(state, requests, countdowns, now + 500);
        expect(view.visible.map((v) => v.key)).toEqual(["resources:20"]);

        // Does not re-pop dismissed card on subsequent server ticks while same request remains
        syncRequestCountdowns(requests, countdowns, now + 1000);
        view = reconcileRequestToastDeck(state, requests, countdowns, now + 1000);
        expect(view.visible.map((v) => v.key)).toEqual(["resources:20"]);
    });

    test("re-allows toast when same player sends a new request after the previous one clears", () => {
        const state = createRequestToastDeckState();
        const countdowns = new Map<string, RequestCountdownRecord>();

        syncRequestCountdowns([allianceReq(7)], countdowns, 1000);
        let view = reconcileRequestToastDeck(state, [allianceReq(7)], countdowns, 1000);
        expect(view.visible.length).toBe(1);

        dismissRequestToastByKey(state, "alliance:7");
        syncRequestCountdowns([], countdowns, 2000);
        view = reconcileRequestToastDeck(state, [], countdowns, 2000);
        expect(view.visible.length).toBe(0);

        // Same player requests again later
        syncRequestCountdowns([allianceReq(7)], countdowns, 3000);
        view = reconcileRequestToastDeck(state, [allianceReq(7)], countdowns, 3000);
        expect(view.visible.length).toBe(1);
    });

    test("auto-expires toast after short TTL without re-showing, and respects shorter request remaining_seconds", () => {
        const state = createRequestToastDeckState();
        const countdowns = new Map<string, RequestCountdownRecord>();
        const requests = [allianceReq(1, 20), allianceReq(2, 3)];

        const now = 50_000;
        syncRequestCountdowns(requests, countdowns, now);
        let view = reconcileRequestToastDeck(state, requests, countdowns, now);
        expect(view.visible.length).toBe(2);

        // At +3.1s, alliance:2 expires from toast (shorter than REQUEST_TOAST_TTL_MS)
        view = reconcileRequestToastDeck(state, requests, countdowns, now + 3100);
        expect(view.visible.map((v) => v.key)).toEqual(["alliance:1"]);

        // At +REQUEST_TOAST_TTL_MS, alliance:1 expires too
        view = reconcileRequestToastDeck(state, requests, countdowns, now + REQUEST_TOAST_TTL_MS + 1);
        expect(view.visible.length).toBe(0);
    });

    test("dismissAll clears deck and suppresses re-appearance for current signatures", () => {
        const state = createRequestToastDeckState();
        const countdowns = new Map<string, RequestCountdownRecord>();
        const requests = [allianceReq(1), resourceReq(2)];

        syncRequestCountdowns(requests, countdowns, 1000);
        reconcileRequestToastDeck(state, requests, countdowns, 1000);
        expect(dismissAllRequestToasts(state)).toBe(true);

        const view = reconcileRequestToastDeck(state, requests, countdowns, 1500);
        expect(view.visible.length).toBe(0);
    });

    test("countdown computes remaining seconds, progress and urgency smoothly", () => {
        const countdowns = new Map<string, RequestCountdownRecord>();
        const req = allianceReq(9, 20);
        const key = requestKey(req);

        syncRequestCountdowns([req], countdowns, 1000);
        const fresh = readRequestCountdown(key, countdowns, 1000)!;
        expect(fresh.remainingSeconds).toBe(20);
        expect(fresh.progress).toBeCloseTo(1, 2);
        expect(fresh.urgent).toBe(false);

        const nearEnd = readRequestCountdown(key, countdowns, 16_500)!;
        expect(nearEnd.remainingSeconds).toBe(5);
        expect(nearEnd.urgent).toBe(true);
        expect(nearEnd.expired).toBe(false);
    });

    test("handles help call requests with 30s default TTL and help:<id> identity", () => {
        const state = createRequestToastDeckState();
        const countdowns = new Map<string, RequestCountdownRecord>();
        const helpReq = (requesterId: number, remainingSeconds = 29): HudInboxRequest => ({
            kind: "help",
            requester_id: requesterId,
            name: `Ally ${requesterId}`,
            avatar: { kind: "procedural", seed: requesterId },
            active: true,
            remaining_seconds: remainingSeconds,
            gold: 250,
            troops: 120,
            pay_gold: 0,
            pay_troops: 0,
        });

        expect(isValidInboxRequest(helpReq(5))).toBe(true);
        expect(requestKey(helpReq(5))).toBe("help:5");

        const req = helpReq(5, 29);
        syncRequestCountdowns([req], countdowns, 10_000);
        const countdown = readRequestCountdown("help:5", countdowns, 10_000)!;
        expect(countdown.remainingSeconds).toBe(29);
        expect(countdown.progress).toBeCloseTo(29 / 30, 2);

        const view = reconcileRequestToastDeck(state, [req], countdowns, 10_000);
        expect(view.visible.map((entry) => entry.key)).toEqual(["help:5"]);

        dismissRequestToastByKey(state, "help:5");
        const afterDismiss = reconcileRequestToastDeck(state, [req], countdowns, 10_100);
        expect(afterDismiss.visible.length).toBe(0);
    });
});
