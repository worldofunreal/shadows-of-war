import * as assert from "node:assert/strict";
import { test } from "node:test";
import { setAvatarPortrait, type AvatarExpressionTransition } from "./avatar-presentation.ts";

function image(initialSource = "old.webp") {
    let source = initialSource;
    const calls: {
        frames: Keyframe[];
        options: KeyframeAnimationOptions;
        cancelled: boolean;
        onfinish: ((event: AnimationPlaybackEvent) => void) | null;
        finish(): void;
    }[] = [];
    const hostStyle = {
        backgroundImage: "",
        backgroundSize: "",
        backgroundPosition: "",
        backgroundRepeat: "",
    };
    const host = { style: hostStyle } as unknown as HTMLElement;
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
            const call = {
                frames,
                options,
                cancelled: false,
                onfinish: null as ((event: AnimationPlaybackEvent) => void) | null,
                finish() {
                    this.onfinish?.({} as AnimationPlaybackEvent);
                },
            };
            calls.push(call);
            return {
                cancel: () => (call.cancelled = true),
                finish: call.finish.bind(call),
                get onfinish() {
                    return call.onfinish;
                },
                set onfinish(value) {
                    call.onfinish = value;
                },
            } as unknown as Animation;
        },
        ownerDocument: {
            defaultView: {
                getComputedStyle(target: HTMLElement | HTMLImageElement) {
                    return target === host
                        ? {
                              backgroundImage: "none",
                              backgroundSize: "auto",
                              backgroundPosition: "0% 0%",
                              backgroundRepeat: "repeat",
                          }
                        : { opacity: "1", transform: "none", objectPosition: "50% 50%" };
                },
            },
        },
        parentElement: host,
    } as unknown as HTMLImageElement;
    return { element, calls, source: () => source, hostStyle };
}

test("avatar portrait crossfades over the outgoing expression without reaching zero opacity", () => {
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
    assert.equal(portrait.calls[0].frames[1].opacity, 1);
    assert.match(String(portrait.calls[0].frames[1].transform), /scale\(\.82\)/);

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
    assert.equal(portrait.calls[1].frames[0].opacity, 0.5);
    assert.equal(portrait.calls[1].frames[1].opacity, 1);
    assert.match(portrait.hostStyle.backgroundImage, /old\.webp/);

    portrait.calls[1].finish();
    assert.equal(portrait.hostStyle.backgroundImage, "");

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
