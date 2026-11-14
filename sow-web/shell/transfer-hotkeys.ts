export type TransferHotkeyAction = "set_max" | "submit" | "close";

export interface TransferHotkeyEvent {
    key: string;
    code?: string;
    ctrlKey?: boolean;
    metaKey?: boolean;
    altKey?: boolean;
    isComposing?: boolean;
}

/** Classifies keyboard shortcuts while the resource transfer/request dialog is open. */
export function resolveTransferHotkey(
    event: TransferHotkeyEvent,
): TransferHotkeyAction | null {
    if (event.ctrlKey || event.metaKey || event.altKey || event.isComposing) {
        return null;
    }
    if (event.key === "Escape") return "close";
    if (event.key === "Enter" || event.code === "NumpadEnter") return "submit";
    if (event.key === "+" || event.key === "=" || event.code === "NumpadAdd") {
        return "set_max";
    }
    return null;
}

export type TransferMaxTarget = "troops" | "gold" | "both";

/** Picks whether `+` maximizes only the focused resource input or both rows. */
export function resolveTransferMaxTarget(
    activeIsTroops: boolean,
    activeIsGold: boolean,
): TransferMaxTarget {
    if (activeIsTroops) return "troops";
    if (activeIsGold) return "gold";
    return "both";
}
