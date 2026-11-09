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
const imageLoads = new Map<string, Promise<HTMLImageElement>>();
const MAX_CACHED_AVATAR_IMAGES = 64;

export function avatarExpressionSheet(path: string): { slug: string; index: number; sheet: string } | null {
    const match = path.match(/(?:^|\/)gameplay\/avatars\/expressions\/([a-z0-9_]+)_(happy|fear|angry)\.webp(?:[?#]|$)/);
    if (!match) return null;
    const index = { happy: 0, fear: 2, angry: 3 }[match[2] as "happy" | "fear" | "angry"];
    return {
        slug: match[1],
        index,
        sheet: "gameplay/avatars/hero-expression-sheets/" + match[1] + ".webp",
    };
}

export function avatarAssetUrl(path: string): string {
    const base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
    const url = base + "/" + path.split("/").map(encodeURIComponent).join("/");
    return window.SOW_assetUrl ? window.SOW_assetUrl(url) : url;
}

export function avatarPortraitPath(slug: string, expression: AvatarExpression | null | undefined): string {
    const portrait =
        expression && expression !== "calm" && ["happy", "fear", "angry"].includes(expression)
            ? "expressions/" + slug + "_" + expression
            : slug;
    return "gameplay/avatars/" + portrait + ".webp";
}

function imageSource(image: HTMLImageElement): string {
    return image.getAttribute("src") || "";
}

function cachedImage(source: string, load: () => Promise<HTMLImageElement>): Promise<HTMLImageElement> {
    const cached = imageLoads.get(source);
    if (cached) {
        imageLoads.delete(source);
        imageLoads.set(source, cached);
        return cached;
    }
    const promise = load();
    imageLoads.set(source, promise);
    while (imageLoads.size > MAX_CACHED_AVATAR_IMAGES) {
        const oldest = imageLoads.keys().next().value;
        if (oldest === undefined) break;
        imageLoads.delete(oldest);
    }
    void promise.catch(() => {
        if (imageLoads.get(source) === promise) imageLoads.delete(source);
    });
    return promise;
}

function loadNetworkImage(source: string): Promise<HTMLImageElement> {
    return cachedImage(source, () => {
        if (typeof Image === "undefined") return Promise.reject(new Error("Image API unavailable"));
        const image = new Image();
        image.crossOrigin = "anonymous";
        image.decoding = "async";
        image.src = source;
        return typeof image.decode === "function"
            ? image.decode().then(() => image)
            : typeof image.addEventListener === "function"
              ? new Promise<HTMLImageElement>((resolve, reject) => {
                    image.addEventListener("load", () => resolve(image), { once: true });
                    image.addEventListener("error", () => reject(new Error("avatar image load failed")), {
                        once: true,
                    });
                })
              : Promise.resolve(image);
    });
}

async function cropExpression(image: HTMLImageElement, index: number): Promise<HTMLImageElement> {
    const side = Math.floor(Math.min(image.naturalWidth, image.naturalHeight) / 2);
    const canvas = document.createElement("canvas");
    canvas.width = side;
    canvas.height = side;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("avatar canvas unavailable");
    context.drawImage(image, (index % 2) * side, Math.floor(index / 2) * side, side, side, 0, 0, side, side);
    const cropped = new Image();
    cropped.decoding = "async";
    cropped.src = canvas.toDataURL("image/webp", 0.9);
    if (typeof cropped.decode === "function") await cropped.decode();
    return cropped;
}

/** One stable browser-native image load shared by the radial and the map atlas bridge. */
export function loadAvatarImage(source: string): Promise<HTMLImageElement> {
    const packed =
        typeof window !== "undefined" && window.SOW_PORTAL === "crazygames"
            ? avatarExpressionSheet(source)
            : null;
    if (!packed) return loadNetworkImage(source);
    return cachedImage(source, async () =>
        cropExpression(await loadNetworkImage(avatarAssetUrl(packed.sheet)), packed.index),
    );
}

/** The map atlas half of the shared loader: the radial's decoded image, scaled to one atlas cell off the main thread. */
export async function loadAvatarBitmap(path: string, size: number): Promise<ImageBitmap> {
    const image = await loadAvatarImage(avatarAssetUrl(path));
    const side = Math.min(image.naturalWidth, image.naturalHeight);
    const sx = (image.naturalWidth - side) / 2;
    const sy = (image.naturalHeight - side) / 2;
    const options: ImageBitmapOptions = { premultiplyAlpha: "none", colorSpaceConversion: "none" };
    const bitmap = await createImageBitmap(image, sx, sy, side, side, {
        ...options,
        resizeWidth: size,
        resizeHeight: size,
        resizeQuality: "high",
    });
    if (bitmap.width === size && bitmap.height === size) return bitmap;
    bitmap.close(); // browsers that ignore the resize options scale on a canvas
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("avatar canvas unavailable");
    context.imageSmoothingQuality = "high";
    context.drawImage(image, sx, sy, side, side, 0, 0, size, size);
    return createImageBitmap(canvas, options);
}

if (typeof window !== "undefined") window.SOW_loadAvatarBitmap = loadAvatarBitmap;

function sourceOf(image: HTMLImageElement): string {
    return imageSource(image);
}

function setSource(image: HTMLImageElement, source: string, deferLoad = false): void {
    if (!source) return;
    if (sourceOf(image) !== source) {
        image.crossOrigin = "anonymous";
        image.src = source;
    }
    if (!deferLoad) void loadAvatarImage(source).catch(() => undefined);
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
    deferLoad = false,
): void {
    const previous = states.get(image);
    const stageKey = transition ? transition.id + ":" + transition.stage : "";

    if (
        previous &&
        previous.key === stageKey &&
        previous.source === source &&
        sourceOf(image) === source
    ) {
        return;
    }
    settleAnimation(image, previous);

    if (deferLoad) {
        const nextSource = transition ? reducedSource : source;
        setSource(image, nextSource, true);
        states.set(image, { key: "", animation: null, source: nextSource, outgoingBackground: null });
        return;
    }

    if (reducedMotion || !transition || typeof image.animate !== "function") {
        if (transition && reducedMotion) source = reducedSource;
        setSource(image, source);
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
        setSource(image, source);
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
    setSource(image, source);
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

/** Stop a deferred portrait request and discard any in-flight expression animation. */
export function clearAvatarPortrait(image: HTMLImageElement): void {
    settleAnimation(image, states.get(image));
    image.removeAttribute("src");
    image.hidden = true;
    states.delete(image);
}
