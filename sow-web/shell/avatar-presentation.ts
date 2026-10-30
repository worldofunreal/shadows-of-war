export type AvatarExpression = "happy" | "calm" | "fear" | "angry";

export interface AvatarExpressionTransition {
    id: number;
    from: AvatarExpression;
    to: AvatarExpression;
    stage: "exit" | "enter";
}

type AnimationState = { key: string; animation: Animation | null; source: string };

const states = new WeakMap<HTMLImageElement, AnimationState>();

function sourceOf(image: HTMLImageElement): string {
    return image.getAttribute("src") || "";
}

function stopAnimation(image: HTMLImageElement, state: AnimationState | undefined): void {
    if (state && state.animation) state.animation.cancel();
}

/** The shared in-match portrait switch: old expression exits, then the new one enters. */
export function setAvatarPortrait(
    image: HTMLImageElement,
    source: string,
    transition: AvatarExpressionTransition | null | undefined,
    reducedMotion: boolean,
    reducedSource: string = source,
): void {
    const previous = states.get(image);
    const stageKey = transition ? transition.id + ":" + transition.stage : "";

    if (reducedMotion || !transition || typeof image.animate !== "function") {
        stopAnimation(image, previous);
        if (transition && reducedMotion) source = reducedSource;
        if (sourceOf(image) !== source && source) image.src = source;
        states.set(image, { key: "", animation: null, source });
        return;
    }

    if (previous && previous.key === stageKey && previous.source === source) return;

    const view = image.ownerDocument && image.ownerDocument.defaultView;
    const computed = view ? view.getComputedStyle(image) : null;
    const currentFrame = {
        opacity: computed ? computed.opacity : "1",
        transform: computed ? computed.transform : "none",
    };
    stopAnimation(image, previous);

    if (transition.stage === "exit") {
        if (source && sourceOf(image) !== source) image.src = source;
        const animation = image.animate(
            [currentFrame, { opacity: 0, transform: "scale(.82) translateY(3px)" }],
            { duration: 150, easing: "ease-in", fill: "both" },
        );
        states.set(image, { key: stageKey, animation, source });
        return;
    }

    if (source && sourceOf(image) !== source) image.src = source;
    const animation = image.animate(
        [
            { opacity: 0, transform: "scale(.82) translateY(3px)" },
            { opacity: 1, transform: "scale(1) translateY(0)" },
        ],
        {
            duration: transition.to === "happy" ? 650 : 500,
            easing: "cubic-bezier(.2,.8,.2,1)",
            fill: "both",
        },
    );
    states.set(image, { key: stageKey, animation, source });
}
