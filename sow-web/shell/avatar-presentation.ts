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

/** A portrait's square cell inside a CrazyGames avatar sheet. */
type AvatarCell = {
    sheet: { url: string; width: number; height: number };
    x: number;
    y: number;
    side: number;
};

const states = new WeakMap<HTMLImageElement, AnimationState>();
/** The portrait an image was last asked to show when that portrait is a packed cell. */
const atlasSources = new WeakMap<HTMLImageElement, string>();
const imageLoads = new Map<string, Promise<HTMLImageElement>>();
const MAX_CACHED_AVATAR_IMAGES = 64;

/** The sheet cell behind a portrait URL or path. Null outside CrazyGames, where portraits are plain files. */
export function avatarAtlasCell(source: string): AvatarCell | null {
    const atlas = typeof window !== "undefined" ? window.SOW_AVATAR_ATLAS : undefined;
    if (!atlas) return null;
    const match = source.match(/(?:^|\/)(gameplay\/avatars\/[^?#]+\.webp)(?:[?#]|$)/);
    const cell = match ? atlas.cells[match[1]] : undefined;
    if (!cell) return null;
    const [sheet, x, y, side] = cell;
    const { path, width, height } = atlas.sheets[sheet];
    return { sheet: { url: avatarAssetUrl(path), width, height }, x, y, side };
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

function escapeAttribute(value: string): string {
    return value.replace(/[&<>"']/g, (char) => "&#" + char.charCodeAt(0) + ";");
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

/** Copies one square cell out of a loaded sheet into an image the page shows like any portrait. */
async function cropCell(sheet: HTMLImageElement, cell: AvatarCell): Promise<HTMLImageElement> {
    const canvas = document.createElement("canvas");
    canvas.width = cell.side;
    canvas.height = cell.side;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("avatar canvas unavailable");
    context.drawImage(sheet, cell.x, cell.y, cell.side, cell.side, 0, 0, cell.side, cell.side);
    const cropped = new Image();
    cropped.decoding = "async";
    cropped.src = canvas.toDataURL("image/webp", 0.9);
    if (typeof cropped.decode === "function") await cropped.decode();
    return cropped;
}

/** One stable browser-native image load shared by the radial and the map atlas bridge. */
export function loadAvatarImage(source: string): Promise<HTMLImageElement> {
    const cell = avatarAtlasCell(source);
    if (!cell) return loadNetworkImage(source);
    return cachedImage(source, async () => cropCell(await loadNetworkImage(cell.sheet.url), cell));
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

/** The portrait an image is meant to show; for a packed cell the displayed src is only its cropped copy. */
function sourceOf(image: HTMLImageElement): string {
    return atlasSources.get(image) ?? imageSource(image);
}

function setSource(image: HTMLImageElement, source: string, deferLoad = false): void {
    if (!source) return;
    if (sourceOf(image) !== source) {
        if (avatarAtlasCell(source)) {
            setAvatarImage(image, source);
        } else {
            atlasSources.delete(image);
            image.crossOrigin = "anonymous";
            image.src = source;
        }
    }
    if (!deferLoad) void loadAvatarImage(source).catch(() => undefined);
}

/** Shows the portrait at `source`. A packed cell is cropped first, so the image never points at a missing file. */
export function setAvatarImage(image: HTMLImageElement, source: string): void {
    if (!avatarAtlasCell(source)) {
        atlasSources.delete(image);
        image.src = source;
        return;
    }
    atlasSources.set(image, source);
    void loadAvatarImage(source)
        .then((portrait) => {
            if (atlasSources.get(image) === source) image.src = portrait.src;
        })
        .catch(() => undefined);
}

let markupWatcher: MutationObserver | null = null;

/** Fills `data-avatar` images in markup once they are inserted. Installed only when sheets are packed. */
function watchAvatarMarkup(): void {
    if (markupWatcher || typeof MutationObserver === "undefined" || typeof document === "undefined") return;
    const fill = (node: Node): void => {
        if (!(node instanceof Element)) return;
        const images: Element[] = node.matches("img[data-avatar]")
            ? [node]
            : Array.from(node.querySelectorAll("img[data-avatar]"));
        for (const image of images) {
            const source = image.getAttribute("data-avatar");
            if (source) setAvatarImage(image as HTMLImageElement, source);
        }
    };
    markupWatcher = new MutationObserver((records) => {
        for (const record of records) {
            if (record.type === "attributes") fill(record.target);
            else record.addedNodes.forEach(fill);
        }
    });
    markupWatcher.observe(document.documentElement, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ["data-avatar"],
    });
}

/** The `src` attribute for an avatar inside an HTML string. Packed cells use `data-avatar`, filled once inserted. */
export function avatarImageAttribute(source: string): string {
    if (!avatarAtlasCell(source)) return "src='" + escapeAttribute(source) + "'";
    watchAvatarMarkup();
    return "data-avatar='" + escapeAttribute(source) + "'";
}

/** CSS declarations that paint the portrait at `source` as a background. A packed cell shows its sheet through background-size and -position. */
export function avatarBackgroundCss(source: string): string {
    const cell = avatarAtlasCell(source);
    if (!cell) return "background-image:url('" + escapeAttribute(source) + "')";
    const { url, width, height } = cell.sheet;
    return (
        "background-image:url('" + escapeAttribute(url) + "');background-repeat:no-repeat;" +
        "background-size:" + percentOf(width, cell.side) + "% " + percentOf(height, cell.side) + "%;" +
        "background-position:" + percentOf(cell.x, width - cell.side) + "% " + percentOf(cell.y, height - cell.side) + "%"
    );
}

/** Paints the portrait at `source` as the background of `element`. */
export function setAvatarBackground(element: HTMLElement, source: string): void {
    const cell = avatarAtlasCell(source);
    if (!cell) {
        element.style.backgroundImage = "url(" + JSON.stringify(source) + ")";
        return;
    }
    const { url, width, height } = cell.sheet;
    element.style.backgroundImage = "url(" + JSON.stringify(url) + ")";
    element.style.backgroundRepeat = "no-repeat";
    element.style.backgroundSize = percentOf(width, cell.side) + "% " + percentOf(height, cell.side) + "%";
    element.style.backgroundPosition =
        percentOf(cell.x, width - cell.side) + "% " + percentOf(cell.y, height - cell.side) + "%";
}

function percentOf(part: number, whole: number): number {
    return whole > 0 ? (part / whole) * 100 : 0;
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

    const outgoingBackground = addOutgoingBackground(image, imageSource(image));
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
    atlasSources.delete(image);
    image.removeAttribute("src");
    image.hidden = true;
    states.delete(image);
}
