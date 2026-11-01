export type AvatarExpression = "happy" | "calm" | "fear" | "angry";

export interface AvatarExpressionTransition {
    id: number;
    from: AvatarExpression;
    to: AvatarExpression;
    stage: "exit" | "enter";
}

type OutgoingBackground = {
    host: HTMLElement;
    image: string;
    size: string;
    position: string;
    repeat: string;
};

type AnimationState = {
    key: string;
    animation: Animation | null;
    source: string;
    outgoingBackground: OutgoingBackground | null;
};

const states = new WeakMap<HTMLImageElement, AnimationState>();

function sourceOf(image: HTMLImageElement): string {
    return image.getAttribute("src") || "";
}

function restoreOutgoingBackground(state: AnimationState): void {
    const background = state.outgoingBackground;
    if (!background) return;
    background.host.style.backgroundImage = background.image;
    background.host.style.backgroundSize = background.size;
    background.host.style.backgroundPosition = background.position;
    background.host.style.backgroundRepeat = background.repeat;
    state.outgoingBackground = null;
}

function settleAnimation(image: HTMLImageElement, state: AnimationState | undefined): void {
    if (!state) return;
    if (state.animation) {
        state.animation.onfinish = null;
        try {
            state.animation.finish();
        } catch {
            // A finished or unsupported animation already has its final frame.
        }
        state.animation.cancel();
    }
    restoreOutgoingBackground(state);
    states.set(image, { key: "", animation: null, source: sourceOf(image), outgoingBackground: null });
}

function addOutgoingBackground(image: HTMLImageElement, source: string): OutgoingBackground | null {
    const host = image.parentElement;
    if (!host || !host.style || !source) return null;

    const view = image.ownerDocument && image.ownerDocument.defaultView;
    const hostStyle = view ? view.getComputedStyle(host) : null;
    const imageStyle = view ? view.getComputedStyle(image) : null;
    const originalImage = hostStyle?.backgroundImage || "none";
    const originalSize = hostStyle?.backgroundSize || "auto";
    const originalPosition = hostStyle?.backgroundPosition || "0% 0%";
    const originalRepeat = hostStyle?.backgroundRepeat || "repeat";
    const outgoing: OutgoingBackground = {
        host,
        image: host.style.backgroundImage,
        size: host.style.backgroundSize,
        position: host.style.backgroundPosition,
        repeat: host.style.backgroundRepeat,
    };
    const layers = originalImage === "none" ? "" : ", " + originalImage;
    host.style.backgroundImage = "url(" + JSON.stringify(source) + ")" + layers;
    host.style.backgroundSize = "cover" + (layers ? ", " + originalSize : "");
    host.style.backgroundPosition =
        (imageStyle?.objectPosition || "50% 50%") + (layers ? ", " + originalPosition : "");
    host.style.backgroundRepeat = "no-repeat" + (layers ? ", " + originalRepeat : "");
    return outgoing;
}

/** The shared in-match portrait switch: keep the outgoing portrait underneath the new one. */
export function setAvatarPortrait(
    image: HTMLImageElement,
    source: string,
    transition: AvatarExpressionTransition | null | undefined,
    reducedMotion: boolean,
    reducedSource: string = source,
): void {
    const previous = states.get(image);
    const stageKey = transition ? transition.id + ":" + transition.stage : "";

    if (previous && previous.key === stageKey && previous.source === source) return;
    settleAnimation(image, previous);

    if (reducedMotion || !transition || typeof image.animate !== "function") {
        if (transition && reducedMotion) source = reducedSource;
        if (sourceOf(image) !== source && source) image.src = source;
        states.set(image, { key: "", animation: null, source, outgoingBackground: null });
        return;
    }

    const view = image.ownerDocument && image.ownerDocument.defaultView;
    const computed = view ? view.getComputedStyle(image) : null;
    const currentFrame = {
        opacity: computed ? computed.opacity : "1",
        transform: computed ? computed.transform : "none",
    };

    if (transition.stage === "exit") {
        if (source && sourceOf(image) !== source) image.src = source;
        const animation = image.animate(
            [currentFrame, { opacity: 1, transform: "scale(.82) translateY(3px)" }],
            { duration: 150, easing: "ease-in", fill: "both" },
        );
        const state: AnimationState = { key: stageKey, animation, source, outgoingBackground: null };
        states.set(image, state);
        animation.onfinish = () => {
            if (states.get(image) === state) restoreOutgoingBackground(state);
        };
        return;
    }

    const outgoingBackground = addOutgoingBackground(image, sourceOf(image));
    if (source && sourceOf(image) !== source) image.src = source;
    const animation = image.animate(
        [
            {
                opacity: outgoingBackground ? 0.5 : 1,
                transform: "scale(.82) translateY(3px)",
            },
            { opacity: 1, transform: "scale(1) translateY(0)" },
        ],
        {
            duration: transition.to === "happy" ? 650 : 500,
            easing: "cubic-bezier(.2,.8,.2,1)",
            fill: "both",
        },
    );
    const state: AnimationState = { key: stageKey, animation, source, outgoingBackground };
    states.set(image, state);
    animation.onfinish = () => {
        if (states.get(image) === state) restoreOutgoingBackground(state);
    };
}
