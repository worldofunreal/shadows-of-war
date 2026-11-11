import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";
import { avatarAtlasCell, avatarBackgroundCss, avatarImageAttribute } from "./avatar-presentation.ts";

const atlas = {
    sheets: [
        { path: "gameplay/atlas/heroes-256-0.webp", width: 512, height: 256 },
        { path: "gameplay/atlas/abilities-128-0.webp", width: 128, height: 128 },
    ],
    cells: {
        "gameplay/avatars/expressions/suppiluliuma_fear.webp": [0, 256, 0, 256],
        "gameplay/avatars/caesar.webp": [0, 0, 0, 256],
        "gameplay/icons/abilities/caesar.webp": [1, 0, 0, 128],
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
    }
    class FakeImage extends FakeElement {
        src = "";
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
        reveal([{ isIntersecting: true, target: image }]);
        await new Promise((resolve) => setTimeout(resolve, 0));
        assert.equal(image.src, "data:image/webp;base64,AAAA");
        assert.deepEqual(watched, [], "a filled avatar is no longer watched");
        assert.equal((drawn[0][0] as { src: string }).src, "/assets/gameplay/atlas/heroes-256-0.webp");
        assert.deepEqual(drawn[0].slice(1), [256, 0, 256, 256, 0, 0, 256, 256]);
    } finally {
        Object.assign(globals, saved);
    }
});
