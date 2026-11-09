import * as assert from "node:assert/strict";
import { test } from "node:test";
import { avatarExpressionSheet } from "./avatar-presentation.ts";

test("CrazyGames hero expressions use their packed 2x2 sheet cells", () => {
    assert.deepEqual(
        ["happy", "fear", "angry"].map((expression) =>
            avatarExpressionSheet(
                `./assets/gameplay/avatars/expressions/suppiluliuma_${expression}.webp?v=123`,
            ),
        ),
        [
            {
                slug: "suppiluliuma",
                index: 0,
                sheet: "gameplay/avatars/hero-expression-sheets/suppiluliuma.webp",
            },
            {
                slug: "suppiluliuma",
                index: 2,
                sheet: "gameplay/avatars/hero-expression-sheets/suppiluliuma.webp",
            },
            {
                slug: "suppiluliuma",
                index: 3,
                sheet: "gameplay/avatars/hero-expression-sheets/suppiluliuma.webp",
            },
        ],
    );
    assert.equal(avatarExpressionSheet("/assets/gameplay/avatars/caesar.webp"), null);
});
