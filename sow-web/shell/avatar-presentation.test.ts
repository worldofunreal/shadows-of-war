import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";
import {
    avatarAtlasCell,
    avatarBackgroundCss,
    avatarImageAttribute,
    deferAvatarImage,
    loadAvatarBitmap,
    loadAvatarImage,
} from "./avatar-presentation.ts";

const atlas = {
    sheets: [
        { path: "gameplay/atlas/heroes-256-0.webp", width: 512, height: 256 },
        { path: "gameplay/atlas/abilities-128-0.webp", width: 128, height: 128 },
        { path: "gameplay/atlas/africa-192-0.webp", width: 384, height: 192 },
        { path: "gameplay/atlas/new-256-0.webp", width: 256, height: 256 },
        { path: "gameplay/atlas/later-256-0.webp", width: 256, height: 256 },
        { path: "gameplay/atlas/map-256-0.webp", width: 512, height: 256 },
    ],
    cells: {
        "gameplay/avatars/expressions/suppiluliuma_fear.webp": [0, 256, 0, 256],
        "gameplay/avatars/caesar.webp": [0, 0, 0, 256],
        "gameplay/icons/abilities/caesar.webp": [1, 0, 0, 128],
        "gameplay/avatars/tribe-chief.webp": [2, 0, 0, 192],
        "gameplay/avatars/later-hero.webp": [4, 0, 0, 256],
        "gameplay/avatars/new-hero.webp": [3, 0, 0, 256],
        "gameplay/avatars/map-hero.webp": [5, 256, 0, 256],
    },
};

function withWindow<T>(config: Record<string, unknown>, run: () => T): T {
    const previous = (globalThis as { window?: unknown }).window;
    (globalThis as { window?: unknown }).window = { SOW_ASSETS_URL: "/assets", ...config };
    try {
        return run();
    } finally {
        (globalThis as { window?: unknown }).window = previous;
    }
}

test("packed sprites resolve to their sheet cell", () => {
    withWindow({ SOW_SPRITE_ATLAS: atlas }, () => {
        assert.deepEqual(avatarAtlasCell("./assets/gameplay/avatars/expressions/suppiluliuma_fear.webp?v=123"), {
            sheet: { url: "/assets/gameplay/atlas/heroes-256-0.webp", width: 512, height: 256 },
            x: 256,
            y: 0,
            side: 256,
        });
        assert.equal(avatarAtlasCell("gameplay/avatars/unlisted.webp"), null);
    });
});

test("packed avatar sheets wait for the first loader to finish without blocking game startup", async () => {
    const globals = globalThis as unknown as Record<string, unknown>;
    const saved = {
        window: globals.window,
        document: globals.document,
        Image: globals.Image,
    };
    const requested: string[] = [];
    class FakeImage {
        crossOrigin = "";
        decoding = "";
        private source = "";
        set src(value: string) {
            this.source = value;
            requested.push(value);
        }
        get src() { return this.source; }
        decode() { return Promise.resolve(); }
    }
    let loaderReady: (() => void) | null = null;
    let loaderHidden = false;
    const canvas = {
        width: 0,
        height: 0,
        getContext: () => ({ drawImage() {} }),
        toDataURL: () => "data:image/webp;base64,AAAA",
    };
    globals.window = {
        SOW_ASSETS_URL: "/assets",
        SOW_SPRITE_ATLAS: atlas,
        addEventListener(name: string, callback: () => void) {
            if (name === "sow:loader-ready") loaderReady = callback;
        },
    };
    globals.document = {
        getElementById: (id: string) =>
            id === "web-loader" ? { getAttribute: () => (loaderHidden ? "true" : "false") } : null,
        createElement: () => canvas,
    };
    globals.Image = FakeImage;
    try {
        const pending = loadAvatarImage("/assets/gameplay/avatars/new-hero.webp");
        await new Promise((resolve) => setTimeout(resolve, 0));
        assert.deepEqual(requested, [], "the optional atlas sheet does not compete with the first tutorial load");

        loaderHidden = true;
        assert.ok(loaderReady, "the avatar loader waits for the first loader-ready event");
        loaderReady();
        const portrait = await pending;
        assert.equal(portrait.src, "data:image/webp;base64,AAAA");
        assert.deepEqual(requested.filter((url) => url.includes("/assets/gameplay/atlas/")), [
            "/assets/gameplay/atlas/new-256-0.webp",
        ]);

        loaderHidden = false;
        const inGamePortrait = loadAvatarImage("/assets/gameplay/avatars/later-hero.webp");
        await new Promise((resolve) => setTimeout(resolve, 0));
        assert.deepEqual(
            requested.filter((url) => url.includes("/assets/gameplay/atlas/")),
            ["/assets/gameplay/atlas/new-256-0.webp", "/assets/gameplay/atlas/later-256-0.webp"],
            "later match loaders do not wait for the one-shot initial loader event",
        );
        await inGamePortrait;
    } finally {
        Object.assign(globals, saved);
    }
});

test("the map atlas cuts a packed cell straight from its decoded sheet, never re-encoding it", async () => {
    const globals = globalThis as unknown as Record<string, unknown>;
    const saved = {
        window: globals.window,
        document: globals.document,
        Image: globals.Image,
        createImageBitmap: globals.createImageBitmap,
    };
    class FakeImage {
        crossOrigin = "";
        decoding = "";
        src = "";
        naturalWidth = 512;
        naturalHeight = 256;
        decode() { return Promise.resolve(); }
    }
    const cuts: unknown[][] = [];
    globals.window = { SOW_ASSETS_URL: "/assets", SOW_SPRITE_ATLAS: atlas };
    globals.document = {
        createElement: () => {
            throw new Error("a packed cell must not go through a canvas");
        },
    };
    globals.Image = FakeImage;
    globals.createImageBitmap = (...args: unknown[]) => {
        cuts.push(args);
        return Promise.resolve({ width: 128, height: 128, close() {} });
    };
    try {
        await loadAvatarBitmap("gameplay/avatars/map-hero.webp", 128);
        assert.equal((cuts[0][0] as FakeImage).src, "/assets/gameplay/atlas/map-256-0.webp");
        assert.deepEqual(cuts[0].slice(1, 5), [256, 0, 256, 256]);
    } finally {
        Object.assign(globals, saved);
    }
});

test("ability icons resolve through the same sheet index as portraits", () => {
    withWindow({ SOW_SPRITE_ATLAS: atlas }, () => {
        assert.deepEqual(avatarAtlasCell("/assets/gameplay/icons/abilities/caesar.webp?v=1"), {
            sheet: { url: "/assets/gameplay/atlas/abilities-128-0.webp", width: 128, height: 128 },
            x: 0,
            y: 0,
            side: 128,
        });
    });
});

test("a sprite outside the atlas keeps its own file and plain markup", () => {
    withWindow({ SOW_SPRITE_ATLAS: atlas }, () => {
        assert.equal(avatarAtlasCell("/assets/gameplay/buildings/atlas.webp"), null);
    });
    withWindow({}, () => {
        assert.equal(avatarAtlasCell("/assets/gameplay/avatars/caesar.webp"), null);
        assert.equal(avatarImageAttribute("/assets/gameplay/avatars/caesar.webp"), "src='/assets/gameplay/avatars/caesar.webp'");
        assert.equal(
            avatarBackgroundCss("/assets/gameplay/avatars/caesar.webp"),
            "background-image:url('/assets/gameplay/avatars/caesar.webp')",
        );
    });
});

test("packed sprites defer to data-avatar and backgrounds show the sheet cell", () => {
    withWindow({ SOW_SPRITE_ATLAS: atlas }, () => {
        assert.equal(
            avatarImageAttribute("/assets/gameplay/avatars/caesar.webp"),
            "data-avatar='/assets/gameplay/avatars/caesar.webp'",
        );
        assert.equal(
            avatarBackgroundCss("/assets/gameplay/avatars/caesar.webp"),
            "background-image:url('/assets/gameplay/atlas/heroes-256-0.webp');background-repeat:no-repeat;" +
                "background-size:200% 100%;background-position:0% 0%",
        );
    });
});

test("no shell module writes a packed avatar file straight into an image src", () => {
    const direct =
        /\bsrc\s*=\s*['"][^;]{0,160}?gameplay\/(?:avatars|icons\/abilities)\/|\.src\s*=[^;]{0,160}?gameplay\/(?:avatars|icons\/abilities)\//;
    assert.match(`<img src='" + esc(asset("gameplay/avatars/caesar.webp")) + "'>`, direct);
    assert.match('image.src = asset("gameplay/avatars/caesar.webp");', direct);
    assert.match(`<img src='" + esc(asset("gameplay/icons/abilities/caesar.webp")) + "'>`, direct);
    const shell = __dirname;
    const offenders = fs
        .readdirSync(shell)
        .filter((file) => file.endsWith(".ts") && !file.endsWith(".test.ts") && file !== "avatar-presentation.ts")
        .filter((file) => direct.test(fs.readFileSync(path.join(shell, file), "utf8")));
    assert.deepEqual(offenders, []);
});

test("data-avatar images are filled with their cropped cell once inserted and visible", async () => {
    const globals = globalThis as unknown as Record<string, unknown>;
    const saved = {
        window: globals.window,
        document: globals.document,
        Element: globals.Element,
        Image: globals.Image,
        MutationObserver: globals.MutationObserver,
        IntersectionObserver: globals.IntersectionObserver,
    };
    class FakeElement {
        attributes: Record<string, string>;
        constructor(attributes: Record<string, string>) {
            this.attributes = attributes;
        }
        matches(selector: string) {
            return selector === "img[data-avatar]" && "data-avatar" in this.attributes;
        }
        getAttribute(name: string) {
            return this.attributes[name] ?? null;
        }
        setAttribute(name: string, value: string) {
            this.attributes[name] = value;
        }
    }
    const requested: string[] = [];
    class FakeImage extends FakeElement {
        private source = "";
        get src() { return this.source; }
        set src(value: string) {
            this.source = value;
            if (value) requested.push(value);
        }
        crossOrigin = "";
        decoding = "";
        decode() {
            return Promise.resolve();
        }
    }
    const drawn: unknown[][] = [];
    const canvas = {
        width: 0,
        height: 0,
        getContext: () => ({ drawImage: (...args: unknown[]) => void drawn.push(args) }),
        toDataURL: () => "data:image/webp;base64,AAAA",
    };
    let deliver: (records: unknown[]) => void = () => undefined;
    globals.window = { SOW_ASSETS_URL: "/assets", SOW_SPRITE_ATLAS: atlas };
    globals.document = { documentElement: {}, createElement: () => canvas };
    globals.Element = FakeElement;
    globals.Image = FakeImage;
    globals.MutationObserver = class {
        constructor(callback: (records: unknown[]) => void) {
            deliver = callback;
        }
        observe() {}
    };
    let reveal: (entries: unknown[]) => void = () => undefined;
    const watched: unknown[] = [];
    globals.IntersectionObserver = class {
        constructor(callback: (entries: unknown[]) => void) {
            reveal = callback;
        }
        observe(target: unknown) {
            watched.push(target);
        }
        unobserve(target: unknown) {
            watched.splice(watched.indexOf(target), 1);
        }
    };
    try {
        const source = "/assets/gameplay/avatars/expressions/suppiluliuma_fear.webp";
        assert.equal(avatarImageAttribute(source), "data-avatar='" + source + "'");
        const image = new FakeImage({ "data-avatar": source });
        deliver([{ type: "childList", addedNodes: [image] }]);
        await new Promise((resolve) => setTimeout(resolve, 0));
        assert.equal(image.src, "", "an inserted avatar waits until it nears the viewport");
        assert.deepEqual(watched, [image]);
        assert.deepEqual(requested, [], "offscreen markup does not fetch sprite sheets");
        reveal([{ isIntersecting: true, target: image }]);
        await new Promise((resolve) => setTimeout(resolve, 0));
        assert.equal(image.src, "data:image/webp;base64,AAAA");
        assert.deepEqual(watched, [], "a filled avatar is no longer watched");
        assert.deepEqual(
            requested.filter((url) => url.includes("/assets/gameplay/atlas/")),
            ["/assets/gameplay/atlas/heroes-256-0.webp"],
            "a visible portrait fetches only its own sheet",
        );
        assert.equal((drawn[0][0] as { src: string }).src, "/assets/gameplay/atlas/heroes-256-0.webp");
        assert.deepEqual(drawn[0].slice(1), [256, 0, 256, 256, 0, 0, 256, 256]);

        const tribeSource = "/assets/gameplay/avatars/tribe-chief.webp";
        const offscreen = new FakeImage({ "data-avatar": "/assets/gameplay/avatars/caesar.webp" });
        deferAvatarImage(offscreen, tribeSource);
        await new Promise((resolve) => setTimeout(resolve, 0));
        assert.equal(offscreen.getAttribute("data-avatar"), tribeSource);
        assert.equal(offscreen.src, "", "changing an offscreen card does not fetch its new sheet");
        assert.deepEqual(
            requested.filter((url) => url.includes("/assets/gameplay/atlas/")),
            ["/assets/gameplay/atlas/heroes-256-0.webp"],
        );
        assert.ok(watched.includes(offscreen), "the changed avatar remains observed until it is visible");
        reveal([{ isIntersecting: true, target: offscreen }]);
        await new Promise((resolve) => setTimeout(resolve, 0));
        assert.equal(offscreen.src, "data:image/webp;base64,AAAA");
        assert.deepEqual(
            requested.filter((url) => url.includes("/assets/gameplay/atlas/")),
            [
                "/assets/gameplay/atlas/heroes-256-0.webp",
                "/assets/gameplay/atlas/africa-192-0.webp",
            ],
            "the newly visible card requests only its region sheet",
        );
    } finally {
        Object.assign(globals, saved);
    }
});
