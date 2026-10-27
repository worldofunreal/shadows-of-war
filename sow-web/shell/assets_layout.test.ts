"use strict";
import * as assert from "node:assert/strict";
import * as crypto from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";

const root = path.join(__dirname, "../..");
const assets = path.join(root, "assets");
const art = path.join(assets, "art");
const MEDIA = /\.(png|webp|jpe?g|svg|gif|mp4|webm|mp3|ogg|ttf|woff2?)$/i;

function files(dir) {
    return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
        const full = path.join(dir, entry.name);
        return entry.isDirectory() ? files(full) : [full];
    });
}

test("art lives only in assets/art; tools write only assets/generated", () => {
    const allowed = ["art", "generated", "fonts", "maps", "map_sources", "site", "campaign"];
    const stray = files(assets)
        .filter((file) => MEDIA.test(file))
        .map((file) => path.relative(assets, file))
        .filter((rel) => !allowed.includes(rel.split(path.sep)[0]));
    assert.deepEqual(stray, [], "media outside the fixed asset folders");
    for (const legacy of ["assets/gameplay", "assets/shell", "sow-tools/assets", "assets/art/badges"]) {
        assert.equal(fs.existsSync(path.join(root, legacy)), false, legacy + " must not come back");
    }
});

test("every art slot the game draws has its file", () => {
    for (const kind of ["city", "factory", "port", "tower", "farm"]) {
        for (const level of [1, 2, 3, 4]) {
            assert.ok(
                ["png", "webp"].some((ext) =>
                    fs.existsSync(path.join(art, "buildings", `${kind}-${level}.${ext}`)),
                ),
                `missing assets/art/buildings/${kind}-${level}.png`,
            );
        }
    }
    for (const unit of ["transport-ship", "trade-ship", "warship", "atom-bomb", "sam-missile"]) {
        assert.ok(
            fs.existsSync(path.join(art, "units", unit + ".png")),
            `missing assets/art/units/${unit}.png`,
        );
    }
    const badges = JSON.parse(fs.readFileSync(path.join(assets, "generated/badges-atlas.json"), "utf8"));
    for (const name of badges.cells) {
        assert.ok(
            ["png", "webp"].some((ext) => fs.existsSync(path.join(art, "icons", `${name}.${ext}`))),
            `missing assets/art/icons/${name}.png`,
        );
    }
    // One file per avatar name, in exactly one folder; each playable hero owns one 2x2 sheet.
    const avatarFolders = ["heroes", "nations", "tribes", "campaign"];
    const avatarNames = new Map();
    for (const folder of avatarFolders) {
        for (const file of fs.readdirSync(path.join(art, "avatars", folder))) {
            const name = path.parse(file).name;
            assert.ok(
                path.extname(file) === ".webp" && /^[a-z][a-z0-9_]*$/.test(name),
                `avatars/${folder}/${file} must be <name>.webp`,
            );
            assert.ok(!avatarNames.has(name), `${name} is in two avatar folders`);
            avatarNames.set(name, folder);
        }
    }
    assert.ok(
        fs.existsSync(path.join(art, "avatars/default.webp")),
        "avatars/default.webp is the face of a player without a portrait",
    );
    const catalog = JSON.parse(fs.readFileSync(path.join(assets, "geo_entities.json"), "utf8")).entities;
    for (const entity of catalog) {
        if (entity.avatar)
            assert.ok(avatarNames.has(entity.avatar), `${entity.id} needs avatars/*/${entity.avatar}.webp`);
    }
    for (const file of fs
        .readdirSync(path.join(assets, "campaign"))
        .filter((name) => name.endsWith(".json") && !name.endsWith(".triggers.json"))) {
        for (const faction of JSON.parse(fs.readFileSync(path.join(assets, "campaign", file), "utf8"))
            .factions) {
            if (faction.avatar && faction.avatar !== "null")
                assert.ok(
                    avatarNames.has(faction.avatar),
                    `${file}: ${faction.id} needs avatars/*/${faction.avatar}.webp`,
                );
        }
    }
    for (const file of fs.readdirSync(path.join(art, "heroes"))) {
        assert.match(
            path.parse(file).name,
            /^[a-z][a-z0-9_]*_(desktop|mobile)$/,
            `unknown hero scene heroes/${file}`,
        );
    }
});

test("the generated building atlas matches the art it was packed from", () => {
    // Changed art without re-running pack-building-atlas (or a hand edit) is caught here.
    const atlas = JSON.parse(fs.readFileSync(path.join(assets, "generated/buildings-atlas.json"), "utf8"));
    for (const [kind, levels] of Object.entries(atlas.sprites)) {
        for (const sprite of Object.values(levels)) {
            const bytes = fs.readFileSync(path.join(art, "buildings", sprite.source));
            assert.equal(
                crypto.createHash("sha256").update(bytes).digest("hex"),
                sprite.sha256,
                kind + " " + sprite.source,
            );
        }
    }
});

test("SOURCES.toml only names folders that exist", () => {
    const sources = fs.readFileSync(path.join(assets, "SOURCES.toml"), "utf8");
    for (const [, rel] of sources.matchAll(/^path = "([^"]+)"/gm)) {
        assert.ok(fs.existsSync(path.join(root, rel)), "SOURCES.toml names a missing folder: " + rel);
    }
});

test("the game draws no emoji: every icon and portrait is art from assets/art", () => {
    // Emoji look different on every device and clash with the pixel art.
    // Tools and the deploy pipeline may print them; the game itself may not.
    const emoji = /[\u{1F000}-\u{1FAFF}\u{2600}-\u{26FF}]/u;
    const gameCrates = [
        "sow-audio",
        "sow-client",
        "sow-core",
        "sow-data",
        "sow-i18n",
        "sow-map",
        "sow-net",
        "sow-relay",
        "sow-render",
        "sow-server",
    ];
    const sources = [
        ...gameCrates.flatMap((crate) =>
            files(path.join(root, crate, "src")).filter((file) => file.endsWith(".rs")),
        ),
        ...files(path.join(root, "sow-web/shell")).filter(
            (file) => /\.(ts|js|css)$/.test(file) && !/\.test\.(ts|js)$/.test(file),
        ),
        ...files(path.join(root, "sow-i18n/strings")).filter((file) => file.endsWith(".toml")),
    ];
    const hits = sources.flatMap((file) =>
        fs
            .readFileSync(file, "utf8")
            .split("\n")
            .flatMap((line, index) =>
                emoji.test(line) ? [`${path.relative(root, file)}:${index + 1}`] : [],
            ),
    );
    assert.deepEqual(hits, []);
});

test("every playable hero is complete: ability, art and texts in every language", () => {
    const catalog = JSON.parse(fs.readFileSync(path.join(assets, "geo_entities.json"), "utf8")).entities;
    const locales = fs.readdirSync(path.join(root, "sow-i18n/strings"));
    const heroes = catalog.filter((entity) => entity.hero).map((entity) => entity.id);
    assert.ok(heroes.length >= 12);
    const heroAvatarDir = path.join(art, "avatars/heroes");
    assert.deepEqual(
        fs.readdirSync(heroAvatarDir).sort(),
        heroes.map((hero) => `${hero}.webp`).sort(),
        "avatars/heroes only contains one spritesheet per playable hero",
    );
    assert.ok(fs.existsSync(path.join(art, "avatars/random.webp")), "Random keeps its shared avatar");
    for (const hero of heroes) {
        const entry = catalog.find((entity) => entity.id === hero);
        assert.ok(entry.hero.ability && entry.hero.name, `${hero} needs a catalog name and ability`);
        const expressionFaces = [1, 2, 3, 4].map((n) => `avatars/heroes/${hero}_${n}.webp`);
        assert.equal(
            entry.hero.expressions,
            undefined,
            `${hero}: expressions are part of its sheet, not catalog metadata`,
        );
        assert.ok(
            !expressionFaces.some((file) => fs.existsSync(path.join(art, file))),
            `${hero} must use one sheet, not separate face files`,
        );
        const heroFiles = [`avatars/heroes/${hero}.webp`, `icons/abilities/${hero}.webp`];
        for (const file of heroFiles) {
            assert.ok(fs.existsSync(path.join(art, file)), `${hero} needs assets/art/${file}`);
        }
        for (const locale of locales) {
            const strings = fs.readFileSync(path.join(root, "sow-i18n/strings", locale, "web.toml"), "utf8");
            for (const field of ["name", "historical", "civilization", "ability", "description"]) {
                const key = `leader_${hero}_${field}`;
                const value = new RegExp(`^${key}\\s*=\\s*"((?:[^"\\\\]|\\\\.)*)"`, "m")
                    .exec(strings)?.[1]
                    ?.trim();
                assert.ok(value, `${key} needs a translation in ${locale}`);
                assert.notEqual(
                    value.toLowerCase(),
                    "name",
                    `${key} must not contain the placeholder in ${locale}`,
                );
                assert.notEqual(
                    value,
                    `[${key}]`,
                    `${key} must not expose a missing-key marker in ${locale}`,
                );
                assert.ok(
                    !/^(?:heroes|site)\.leader_/.test(value),
                    `${key} must not expose a raw translation key in ${locale}`,
                );
            }
        }
    }
});
