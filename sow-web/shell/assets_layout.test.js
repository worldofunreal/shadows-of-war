"use strict";
const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

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
    for (const legacy of ["assets/gameplay", "assets/shell", "sow-tools/assets"]) {
        assert.equal(fs.existsSync(path.join(root, legacy)), false, legacy + " must not come back");
    }
});

test("every art slot the game draws has its file", () => {
    for (const kind of ["city", "factory", "port", "bunker", "farm"]) {
        for (const level of [1, 2, 3, 4]) {
            assert.ok(
                ["png", "webp"].some((ext) => fs.existsSync(path.join(art, "buildings", `${kind}-${level}.${ext}`))),
                `missing assets/art/buildings/${kind}-${level}.png`
            );
        }
    }
    for (const unit of ["transport-ship", "trade-ship", "warship", "atom-bomb", "sam-missile"]) {
        assert.ok(fs.existsSync(path.join(art, "units", unit + ".png")), `missing assets/art/units/${unit}.png`);
    }
    const badges = JSON.parse(fs.readFileSync(path.join(assets, "generated/badges-atlas.json"), "utf8"));
    for (const name of badges.cells) {
        assert.ok(fs.existsSync(path.join(art, "badges", name + ".png")), `missing assets/art/badges/${name}.png`);
    }
    const commerce = fs.readFileSync(path.join(root, "sow-data/src/commerce.rs"), "utf8");
    const leaderIds = commerce.slice(commerce.indexOf("pub fn leader_id"), commerce.indexOf("pub fn leader_wire_id"));
    const slugs = [...leaderIds.matchAll(/=> "([a-z_]+)"/g)].map((match) => match[1]);
    assert.ok(slugs.length >= 12);
    for (const slug of slugs) {
        for (const slot of ["portrait", "expressions", "scene-desktop", "scene-mobile"]) {
            assert.ok(fs.existsSync(path.join(art, "leaders", slug, slot + ".webp")), `missing ${slug}/${slot}`);
        }
    }
    const slots = new Set(["portrait", "expressions", "scene-desktop", "scene-mobile"]);
    for (const leader of fs.readdirSync(path.join(art, "leaders"))) {
        for (const file of fs.readdirSync(path.join(art, "leaders", leader))) {
            assert.ok(slots.has(path.parse(file).name), `unknown leader art ${leader}/${file}`);
        }
    }
});

test("the generated building atlas matches the art it was packed from", () => {
    // Changed art without re-running pack-building-atlas (or a hand edit) is caught here.
    const atlas = JSON.parse(fs.readFileSync(path.join(assets, "generated/buildings-atlas.json"), "utf8"));
    assert.equal(atlas.atlas, "gameplay/buildings/atlas.webp");
    for (const [kind, levels] of Object.entries(atlas.sprites)) {
        for (const sprite of Object.values(levels)) {
            const bytes = fs.readFileSync(path.join(art, "buildings", sprite.source));
            assert.equal(crypto.createHash("sha256").update(bytes).digest("hex"), sprite.sha256, kind + " " + sprite.source);
        }
    }
});

test("SOURCES.toml only names folders that exist", () => {
    const sources = fs.readFileSync(path.join(assets, "SOURCES.toml"), "utf8");
    for (const [, rel] of sources.matchAll(/^path = "([^"]+)"/gm)) {
        assert.ok(fs.existsSync(path.join(root, rel)), "SOURCES.toml names a missing folder: " + rel);
    }
});
