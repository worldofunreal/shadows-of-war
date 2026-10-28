import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { test } from "node:test";

test("committed admin analytics script is the build of its TypeScript source", async () => {
  const built = await Bun.build({
    entrypoints: [path.join(__dirname, "admin_analytics.ts")],
    target: "browser",
    format: "iife",
  });
  assert.equal(built.success, true);
  const committed = fs.readFileSync(path.join(__dirname, "admin_analytics.js"), "utf8");
  assert.equal(committed, await built.outputs[0].text());
});
