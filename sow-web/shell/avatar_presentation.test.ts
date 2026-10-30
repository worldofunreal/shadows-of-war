import * as assert from "node:assert/strict";
import { test } from "node:test";
import { setAvatarPortrait, type AvatarExpressionTransition } from "./avatar-presentation.ts";

function image(initialSource = "old.webp") {
    let source = initialSource;
    const calls: { frames: Keyframe[]; options: KeyframeAnimationOptions; cancelled: boolean }[] = [];
    const element = {
        getAttribute(name: string) {
            return name === "src" ? source : null;
        },
        set src(value: string) {
            source = value;
        },
        get src() {
            return source;
        },
        animate(frames: Keyframe[], options: KeyframeAnimationOptions) {
            const call = { frames, options, cancelled: false };
            calls.push(call);
            return { cancel: () => (call.cancelled = true) } as unknown as Animation;
        },
        ownerDocument: null,
    } as unknown as HTMLImageElement;
    return { element, calls, source: () => source };
}

test("avatar portrait changes stage exit before entry and ignores duplicate phase updates", () => {
    const portrait = image();
    const transition: AvatarExpressionTransition = {
        id: 7,
        from: "calm",
        to: "angry",
        stage: "exit",
    };

    setAvatarPortrait(portrait.element, "old.webp", transition, false, "new.webp");
    assert.equal(portrait.source(), "old.webp");
    assert.equal(portrait.calls.length, 1);
    assert.equal(portrait.calls[0].options.duration, 150);

    setAvatarPortrait(portrait.element, "old.webp", transition, false, "new.webp");
    assert.equal(portrait.calls.length, 1);

    setAvatarPortrait(
        portrait.element,
        "new.webp",
        { ...transition, stage: "enter" },
        false,
        "new.webp",
    );
    assert.equal(portrait.source(), "new.webp");
    assert.equal(portrait.calls.length, 2);
    assert.equal(portrait.calls[1].options.duration, 500);
    assert.equal(portrait.calls[0].cancelled, true);

    setAvatarPortrait(portrait.element, "new.webp", null, false);
    assert.equal(portrait.calls[1].cancelled, true);
});

test("reduced motion applies the final expression without animation", () => {
    const portrait = image();
    setAvatarPortrait(
        portrait.element,
        "old.webp",
        { id: 1, from: "calm", to: "fear", stage: "exit" },
        true,
        "fear.webp",
    );
    assert.equal(portrait.source(), "fear.webp");
    assert.equal(portrait.calls.length, 0);
});
