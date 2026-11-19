import type { HudInboxRequest } from "./main_menu.state.ts";

export const MAX_REQUEST_TOAST_CARDS = 4;
export const DESKTOP_MAX_REQUEST_TOASTS = 3;
export const COMPACT_HORIZONTAL_MAX_REQUEST_TOASTS = 2;
export const MOBILE_MAX_REQUEST_TOASTS = 1;
export const REQUEST_TOAST_TTL_MS = 7000;
export const REQUEST_URGENT_THRESHOLD_SECS = 5;

/** Picks how many floating request cards fit side-by-side in 3D stack for this viewport. */
export function resolveRequestToastMaxVisible(
    width: number,
    height: number,
    compactLandscape = false,
): number {
    const w = Number(width);
    const h = Number(height);
    if (Number.isFinite(w) && Number.isFinite(h) && w > 0 && h > 0) {
        const isPortrait = w <= h;
        if (w <= 720 || (isPortrait && w <= 899)) {
            return MOBILE_MAX_REQUEST_TOASTS;
        }
        if (compactLandscape || (w > h && (w < 960 || h < 720))) {
            return COMPACT_HORIZONTAL_MAX_REQUEST_TOASTS;
        }
        return DESKTOP_MAX_REQUEST_TOASTS;
    }
    return compactLandscape
        ? COMPACT_HORIZONTAL_MAX_REQUEST_TOASTS
        : DESKTOP_MAX_REQUEST_TOASTS;
}

export interface RequestCountdownRecord {
    signature: string;
    expiresAtMs: number;
    ttlMs: number;
}

export interface RequestCountdownSnapshot {
    remainingMs: number;
    remainingSeconds: number;
    progress: number;
    urgent: boolean;
    expired: boolean;
}

export interface RequestToastEntry {
    key: string;
    signature: string;
    request: HudInboxRequest;
    toastExpiresAtMs: number;
    createdAtMs: number;
}

export interface RequestToastDeckState {
    entries: RequestToastEntry[];
    knownSignatures: Map<string, string>;
    dismissedSignatures: Set<string>;
}

export interface RequestToastDeckView {
    visible: RequestToastEntry[];
    overflowCount: number;
    totalPending: number;
    nextExpiryMs: number | null;
}

export function createRequestToastDeckState(): RequestToastDeckState {
    return {
        entries: [],
        knownSignatures: new Map<string, string>(),
        dismissedSignatures: new Set<string>(),
    };
}

const EMPTY_ENTRIES: RequestToastEntry[] = [];
const EMPTY_DECK_VIEW: RequestToastDeckView = {
    visible: EMPTY_ENTRIES,
    overflowCount: 0,
    totalPending: 0,
    nextExpiryMs: null,
};
const scratchActiveKeys = new Set<string>();
const scratchCurrentByKey = new Map<string, { request: HudInboxRequest; signature: string }>();
const scratchActiveSignatures = new Set<string>();
const scratchIncomingKeys = new Set<string>();

export function isValidInboxRequest(request: HudInboxRequest | null | undefined): request is HudInboxRequest {
    return Boolean(
        request &&
            (request.kind === "alliance" || request.kind === "resources" || request.kind === "help") &&
            Number.isSafeInteger(request.requester_id) &&
            request.requester_id > 0,
    );
}

/** Floating toast popups are restricted to alliance proposals/renewals so aid requests stay quiet in Inbox. */
export function isPopupEligibleRequest(
    request: HudInboxRequest | null | undefined,
): request is HudInboxRequest {
    return Boolean(isValidInboxRequest(request) && request.kind === "alliance");
}

export function requestKey(request: HudInboxRequest): string {
    return request.kind + ":" + request.requester_id;
}

function wholeMetric(value: unknown): number {
    const num = Number(value);
    return Number.isFinite(num) ? Math.max(0, Math.floor(num)) : 0;
}

export function requestSignature(request: HudInboxRequest): string {
    if (request.kind === "alliance") {
        return [
            request.kind,
            request.requester_id,
            request.is_renewal ? 1 : 0,
        ].join(":");
    }
    if (request.kind === "help") {
        const callId = Number(request.call_id);
        return Number.isFinite(callId) && callId > 0
            ? `help:${request.requester_id}:${Math.floor(callId)}`
            : `help:${request.requester_id}`;
    }
    return [
        request.kind,
        request.requester_id,
        wholeMetric(request.gold),
        wholeMetric(request.troops),
        wholeMetric(request.pay_gold),
        wholeMetric(request.pay_troops),
    ].join(":");
}

export function syncRequestCountdowns(
    requests: HudInboxRequest[],
    countdowns: Map<string, RequestCountdownRecord>,
    nowMs: number,
): void {
    if (requests.length === 0) {
        if (countdowns.size > 0) countdowns.clear();
        return;
    }
    scratchActiveKeys.clear();
    for (let i = 0; i < requests.length; i++) {
        const request = requests[i];
        if (!isValidInboxRequest(request)) continue;
        const key = requestKey(request);
        if (scratchActiveKeys.has(key)) continue;
        scratchActiveKeys.add(key);
        const remainingSecs = Number(request.remaining_seconds);
        if (!Number.isFinite(remainingSecs)) {
            countdowns.delete(key);
            continue;
        }
        const remainingMs = Math.max(0, remainingSecs * 1000);
        const ttlSecs = Number(request.ttl_seconds);
        const defaultTtlSecs = request.kind === "resources" || request.kind === "help" ? 30 : 20;
        const ttlMs = Math.max(
            1000,
            (Number.isFinite(ttlSecs) && ttlSecs > 0 ? ttlSecs : defaultTtlSecs) * 1000,
        );
        const signature = requestSignature(request);
        const nextExpiresAtMs = nowMs + remainingMs;
        const existing = countdowns.get(key);
        if (existing && existing.signature === signature) {
            existing.ttlMs = ttlMs;
            // Re-anchor only on meaningful drift so local ticks stay smooth.
            if (Math.abs(existing.expiresAtMs - nextExpiresAtMs) > 1100) {
                existing.expiresAtMs = nextExpiresAtMs;
            }
        } else {
            countdowns.set(key, {
                signature,
                expiresAtMs: nextExpiresAtMs,
                ttlMs,
            });
        }
    }
    for (const key of countdowns.keys()) {
        if (!scratchActiveKeys.has(key)) {
            countdowns.delete(key);
        }
    }
    scratchActiveKeys.clear();
}

export function readRequestCountdown(
    key: string,
    countdowns: Map<string, RequestCountdownRecord>,
    nowMs: number,
): RequestCountdownSnapshot | null {
    const record = countdowns.get(key);
    if (!record) return null;
    const remainingMs = Math.max(0, record.expiresAtMs - nowMs);
    const remainingSeconds = Math.ceil(remainingMs / 1000);
    const progress = Math.max(0, Math.min(1, remainingMs / record.ttlMs));
    return {
        remainingMs,
        remainingSeconds,
        progress,
        urgent: remainingSeconds <= REQUEST_URGENT_THRESHOLD_SECS,
        expired: remainingMs <= 0,
    };
}

export function reconcileRequestToastDeck(
    state: RequestToastDeckState,
    requests: HudInboxRequest[],
    countdowns: Map<string, RequestCountdownRecord>,
    nowMs: number,
    options: {
        inboxOpen?: boolean;
        toastTtlMs?: number;
        maxVisible?: number;
        filter?: (request: HudInboxRequest) => boolean;
    } = {},
): RequestToastDeckView {
    const filter = options.filter ?? isPopupEligibleRequest;
    if (state.entries.length === 0 && requests.length === 0) {
        if (state.knownSignatures.size > 0) state.knownSignatures.clear();
        if (state.dismissedSignatures.size > 0) state.dismissedSignatures.clear();
        return EMPTY_DECK_VIEW;
    }

    const toastTtlMs = Math.max(1000, options.toastTtlMs ?? REQUEST_TOAST_TTL_MS);
    scratchCurrentByKey.clear();
    scratchActiveSignatures.clear();
    for (let i = 0; i < requests.length; i++) {
        const request = requests[i];
        if (!filter(request)) continue;
        const key = requestKey(request);
        if (scratchCurrentByKey.has(key)) continue;
        const sig = requestSignature(request);
        scratchCurrentByKey.set(key, { request, signature: sig });
        scratchActiveSignatures.add(sig);
    }

    if (scratchCurrentByKey.size === 0 && state.entries.length === 0) {
        if (state.knownSignatures.size > 0) state.knownSignatures.clear();
        if (state.dismissedSignatures.size > 0) state.dismissedSignatures.clear();
        scratchCurrentByKey.clear();
        scratchActiveSignatures.clear();
        return EMPTY_DECK_VIEW;
    }

    for (const sig of state.dismissedSignatures) {
        if (!scratchActiveSignatures.has(sig)) {
            state.dismissedSignatures.delete(sig);
        }
    }

    const retained: RequestToastEntry[] = [];
    for (let i = 0; i < state.entries.length; i++) {
        const entry = state.entries[i];
        const live = scratchCurrentByKey.get(entry.key);
        if (!live) continue;
        if (live.signature !== entry.signature) continue;
        if (state.dismissedSignatures.has(entry.signature)) continue;
        if (entry.toastExpiresAtMs <= nowMs) {
            state.dismissedSignatures.add(entry.signature);
            continue;
        }
        retained.push({
            ...entry,
            request: live.request,
        });
    }

    const incoming: RequestToastEntry[] = [];
    scratchIncomingKeys.clear();
    for (const [key, live] of scratchCurrentByKey.entries()) {
        const prevSig = state.knownSignatures.get(key);
        const isNewOrUpdated = !prevSig || prevSig !== live.signature;
        if (!isNewOrUpdated) continue;
        if (state.dismissedSignatures.has(live.signature)) continue;
        if (options.inboxOpen) {
            state.dismissedSignatures.add(live.signature);
            continue;
        }
        const countdown = countdowns.get(key);
        const requestRemainingMs = countdown
            ? Math.max(0, countdown.expiresAtMs - nowMs)
            : toastTtlMs;
        if (requestRemainingMs <= 0) continue;
        const durationMs = Math.min(toastTtlMs, requestRemainingMs);
        scratchIncomingKeys.add(key);
        incoming.push({
            key,
            signature: live.signature,
            request: live.request,
            toastExpiresAtMs: nowMs + durationMs,
            createdAtMs: nowMs,
        });
    }

    const combined: RequestToastEntry[] =
        incoming.length > 0 ? incoming.slice() : [];
    for (let i = 0; i < retained.length; i++) {
        const entry = retained[i];
        if (!scratchIncomingKeys.has(entry.key)) {
            combined.push(entry);
        }
    }
    scratchIncomingKeys.clear();

    state.knownSignatures.clear();
    for (const [key, live] of scratchCurrentByKey.entries()) {
        state.knownSignatures.set(key, live.signature);
    }
    const totalPending = scratchCurrentByKey.size;
    scratchCurrentByKey.clear();
    scratchActiveSignatures.clear();

    const rawLimit = Number(options.maxVisible);
    const maxVisible = Number.isFinite(rawLimit)
        ? Math.max(1, Math.min(MAX_REQUEST_TOAST_CARDS, Math.floor(rawLimit)))
        : MAX_REQUEST_TOAST_CARDS;
    if (combined.length > maxVisible) {
        for (let i = maxVisible; i < combined.length; i++) {
            // Evicted overflow cards disappear from floating toasts ("once and bye") and stay in Inbox.
            state.dismissedSignatures.add(combined[i].signature);
        }
        combined.length = maxVisible;
    }
    state.entries = combined;
    let nextExpiryMs: number | null = null;
    for (let i = 0; i < state.entries.length; i++) {
        const expiresAt = state.entries[i].toastExpiresAtMs;
        if (nextExpiryMs === null || expiresAt < nextExpiryMs) {
            nextExpiryMs = expiresAt;
        }
    }

    return {
        visible: combined,
        overflowCount: Math.max(0, totalPending - combined.length),
        totalPending,
        nextExpiryMs,
    };
}

export function dismissFrontRequestToast(state: RequestToastDeckState): boolean {
    if (!state.entries.length) return false;
    const front = state.entries.shift()!;
    state.dismissedSignatures.add(front.signature);
    return true;
}

export function dismissRequestToastByKey(state: RequestToastDeckState, key: string): boolean {
    const index = state.entries.findIndex((entry) => entry.key === key);
    if (index < 0) return false;
    const [removed] = state.entries.splice(index, 1);
    state.dismissedSignatures.add(removed.signature);
    return true;
}

export function dismissAllRequestToasts(state: RequestToastDeckState): boolean {
    if (!state.entries.length) return false;
    for (const entry of state.entries) {
        state.dismissedSignatures.add(entry.signature);
    }
    state.entries = [];
    return true;
}

export function resetRequestToastDeck(state: RequestToastDeckState): void {
    state.entries = [];
    state.knownSignatures.clear();
    state.dismissedSignatures.clear();
}
