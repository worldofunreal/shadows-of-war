import type { HudInboxRequest } from "./main_menu.state.ts";

export const MAX_REQUEST_TOAST_CARDS = 4;
export const REQUEST_TOAST_TTL_MS = 7000;
export const REQUEST_URGENT_THRESHOLD_SECS = 5;

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

export function isValidInboxRequest(request: HudInboxRequest | null | undefined): request is HudInboxRequest {
    return Boolean(
        request &&
            (request.kind === "alliance" || request.kind === "resources" || request.kind === "help") &&
            Number.isSafeInteger(request.requester_id) &&
            request.requester_id > 0,
    );
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
    const activeKeys = new Set<string>();
    for (const request of requests) {
        if (!isValidInboxRequest(request)) continue;
        const key = requestKey(request);
        if (activeKeys.has(key)) continue;
        activeKeys.add(key);
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
    for (const key of Array.from(countdowns.keys())) {
        if (!activeKeys.has(key)) {
            countdowns.delete(key);
        }
    }
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
    options: { inboxOpen?: boolean; toastTtlMs?: number } = {},
): RequestToastDeckView {
    const toastTtlMs = Math.max(1000, options.toastTtlMs ?? REQUEST_TOAST_TTL_MS);
    const currentByKey = new Map<string, { request: HudInboxRequest; signature: string }>();
    const activeSignatures = new Set<string>();
    for (const request of requests) {
        if (!isValidInboxRequest(request)) continue;
        const key = requestKey(request);
        if (currentByKey.has(key)) continue;
        const sig = requestSignature(request);
        currentByKey.set(key, { request, signature: sig });
        activeSignatures.add(sig);
    }

    for (const sig of Array.from(state.dismissedSignatures)) {
        if (!activeSignatures.has(sig)) {
            state.dismissedSignatures.delete(sig);
        }
    }

    const retained: RequestToastEntry[] = [];
    for (const entry of state.entries) {
        const live = currentByKey.get(entry.key);
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
    for (const [key, live] of currentByKey.entries()) {
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
        incoming.push({
            key,
            signature: live.signature,
            request: live.request,
            toastExpiresAtMs: nowMs + durationMs,
            createdAtMs: nowMs,
        });
    }

    const incomingKeys = new Set(incoming.map((entry) => entry.key));
    const filteredRetained = retained.filter((entry) => !incomingKeys.has(entry.key));
    state.entries = [...incoming, ...filteredRetained];

    state.knownSignatures.clear();
    for (const [key, live] of currentByKey.entries()) {
        state.knownSignatures.set(key, live.signature);
    }

    const visible = state.entries.slice(0, MAX_REQUEST_TOAST_CARDS);
    let nextExpiryMs: number | null = null;
    for (const entry of state.entries) {
        if (nextExpiryMs === null || entry.toastExpiresAtMs < nextExpiryMs) {
            nextExpiryMs = entry.toastExpiresAtMs;
        }
    }

    return {
        visible,
        overflowCount: Math.max(0, state.entries.length - visible.length),
        totalPending: currentByKey.size,
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
