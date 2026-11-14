import { describe, expect, test } from "bun:test";
import {
    resolveTransferHotkey,
    resolveTransferMaxTarget,
} from "./transfer-hotkeys.ts";

describe("resolveTransferHotkey", () => {
    test("maps +, =, and NumpadAdd to set_max", () => {
        expect(resolveTransferHotkey({ key: "+" })).toBe("set_max");
        expect(resolveTransferHotkey({ key: "=", code: "Equal" })).toBe("set_max");
        expect(resolveTransferHotkey({ key: "Unidentified", code: "NumpadAdd" })).toBe("set_max");
    });

    test("maps Enter and NumpadEnter to submit, and Escape to close", () => {
        expect(resolveTransferHotkey({ key: "Enter" })).toBe("submit");
        expect(resolveTransferHotkey({ key: "Enter", code: "NumpadEnter" })).toBe("submit");
        expect(resolveTransferHotkey({ key: "Escape" })).toBe("close");
    });

    test("ignores modified or IME composing events and unrelated keys", () => {
        expect(resolveTransferHotkey({ key: "+", ctrlKey: true })).toBeNull();
        expect(resolveTransferHotkey({ key: "Enter", metaKey: true })).toBeNull();
        expect(resolveTransferHotkey({ key: "+", altKey: true })).toBeNull();
        expect(resolveTransferHotkey({ key: "Enter", isComposing: true })).toBeNull();
        expect(resolveTransferHotkey({ key: "5" })).toBeNull();
    });
});

describe("resolveTransferMaxTarget", () => {
    test("maximizes focused resource row or both rows when neither input is focused", () => {
        expect(resolveTransferMaxTarget(true, false)).toBe("troops");
        expect(resolveTransferMaxTarget(false, true)).toBe("gold");
        expect(resolveTransferMaxTarget(false, false)).toBe("both");
    });
});
