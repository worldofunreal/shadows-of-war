import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { spawn, spawnSync } from "node:child_process";

const ROOT = path.resolve(import.meta.dir, "..");
const SHELL_DIR = path.join(ROOT, "sow-web", "shell");
const OUT_DIR = path.join(ROOT, "dist", "native-dev");
const PLAY_DIR = path.join(OUT_DIR, "play");
const LOCALES_OUT = path.join(OUT_DIR, "locales");
const SDK_OUT = path.join(OUT_DIR, "sdk");

const args = new Set(process.argv.slice(2));
const forceWasm = args.has("--wasm");
const watchRust = args.has("--watch-rust");
const dryRun = args.has("--dry-run");
const startMobile = args.has("--mobile");

const CSS_FILES = [
  "sow-controls.css",
  "main_menu.base.css",
  "main_menu.layout.css",
  "main_menu.lobbies.css",
  "main_menu.create.css",
  "main_menu.queue.css",
  "main_menu.settings.css",
  "main_menu.auth.css",
  "main_menu.campaign.css",
  "main_menu.hud.css",
  "main_menu.tutorial.css",
  "main_menu.profile.css",
  "main_menu.missions.css",
];

function findLiveAssetsDir(): string {
  const candidates = [
    path.join(ROOT, "dist", "web", "assets"),
    path.join(ROOT, "dist", ".sow-state", "local-runtime-assets"),
    path.join(ROOT, "dist", ".sow-state", "prod-runtime-assets"),
  ];
  for (const dir of candidates) {
    if (fs.existsSync(path.join(dir, "gameplay")) && fs.existsSync(path.join(dir, "shell"))) {
      return dir;
    }
  }
  return candidates[0];
}

function resolveElectronBin(): string {
  for (const candidate of [
    process.env.SOW_ELECTRON,
    "/usr/bin/electron44",
    "/usr/bin/electron",
    "/usr/local/bin/electron",
  ]) {
    if (candidate && fs.existsSync(candidate)) return candidate;
  }
  throw new Error("Electron executable not found (/usr/bin/electron44 or /usr/bin/electron).");
}

function readWebCatalogVersion(): string {
  const libRs = fs.readFileSync(path.join(ROOT, "sow-i18n", "src", "lib.rs"), "utf8");
  const match = libRs.match(/WEB_CATALOG_VERSION:\s*u32\s*=\s*(\d+)/);
  return match ? match[1] : "1";
}

function compileTs(entryRel: string): Promise<string> {
  const entry = path.join(SHELL_DIR, entryRel);
  return Bun.build({
    entrypoints: [entry],
    target: "browser",
    format: "iife",
    minify: false,
  }).then(async (res) => {
    if (!res.success) {
      const msg = res.logs.map((l) => l.message).join("\n");
      throw new Error(`Bun.build failed for ${entryRel}:\n${msg}`);
    }
    return await res.outputs[0].text();
  });
}

function ensureWasm(rebuild = false): { jsFile: string; wasmFile: string } {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  const rawWasm = path.join(ROOT, "target", "wasm32-unknown-unknown", "wasm-dev", "sow_client.wasm");
  const devJs = "sow_client_dev.js";
  const devWasm = "sow_client_dev_bg.wasm";
  const devJsPath = path.join(OUT_DIR, devJs);
  const devWasmPath = path.join(OUT_DIR, devWasm);

  const shouldCompileRust = rebuild || (!fs.existsSync(rawWasm) && !findDistWebWasm());
  if (shouldCompileRust) {
    console.log("==> Compiling uncompressed WASM (wasm-dev)...");
    const res = spawnSync(
      "cargo",
      ["build", "--profile", "wasm-dev", "-p", "sow-client", "--features", "dev", "--target", "wasm32-unknown-unknown"],
      {
        cwd: ROOT,
        stdio: "inherit",
        env: {
          ...process.env,
          RUSTFLAGS: "-C target-feature=-bulk-memory",
          CARGO_BUILD_JOBS: "4",
        },
      },
    );
    if (res.status !== 0) throw new Error("cargo build (wasm-dev) failed");
  }

  if (fs.existsSync(rawWasm)) {
    const rawMtime = fs.statSync(rawWasm).mtimeMs;
    const outMtime = fs.existsSync(devWasmPath) ? fs.statSync(devWasmPath).mtimeMs : 0;
    if (shouldCompileRust || rawMtime > outMtime || !fs.existsSync(devJsPath)) {
      console.log("==> Running wasm-bindgen (uncompressed)...");
      const bindgen = spawnSync(
        "wasm-bindgen",
        [rawWasm, "--target", "web", "--out-dir", OUT_DIR, "--out-name", "sow_client_dev", "--no-typescript"],
        { cwd: ROOT, stdio: "inherit" },
      );
      if (bindgen.status !== 0) throw new Error("wasm-bindgen failed");
    }
    return { jsFile: devJs, wasmFile: devWasm };
  }

  const distWasm = findDistWebWasm();
  if (distWasm) {
    if (!fs.existsSync(devJsPath)) fs.copyFileSync(distWasm.jsPath, devJsPath);
    if (!fs.existsSync(devWasmPath)) fs.copyFileSync(distWasm.wasmPath, devWasmPath);
    return { jsFile: devJs, wasmFile: devWasm };
  }

  throw new Error("No WASM binary found. Run with --wasm to build wasm-dev.");
}

function findDistWebWasm(): { jsPath: string; wasmPath: string } | null {
  const distWeb = path.join(ROOT, "dist", "web");
  if (!fs.existsSync(distWeb)) return null;
  const files = fs.readdirSync(distWeb);
  const wasmName = files.find((f) => f.startsWith("sow_client_") && f.endsWith("_bg.wasm"));
  if (!wasmName) return null;
  const jsName = wasmName.replace(/_bg\.wasm$/, ".js");
  const jsPath = path.join(distWeb, jsName);
  const wasmPath = path.join(distWeb, wasmName);
  if (!fs.existsSync(jsPath) || !fs.existsSync(wasmPath)) return null;
  return { jsPath, wasmPath };
}

function deepMergeStrings(
  target: Record<string, unknown>,
  source: Record<string, unknown>,
): Record<string, unknown> {
  const out: Record<string, unknown> = { ...target };
  for (const [k, v] of Object.entries(source)) {
    const existing = out[k];
    if (
      v &&
      typeof v === "object" &&
      !Array.isArray(v) &&
      existing &&
      typeof existing === "object" &&
      !Array.isArray(existing)
    ) {
      out[k] = deepMergeStrings(existing as Record<string, unknown>, v as Record<string, unknown>);
    } else {
      out[k] = v;
    }
  }
  return out;
}

function buildLocales(liveAssetsDir: string): string[] {
  fs.mkdirSync(LOCALES_OUT, { recursive: true });
  const distLocales = path.join(ROOT, "dist", "web", "locales");
  if (fs.existsSync(distLocales)) {
    for (const entry of fs.readdirSync(distLocales)) {
      const src = path.join(distLocales, entry);
      const dst = path.join(LOCALES_OUT, entry);
      if (fs.statSync(src).isFile()) {
        fs.copyFileSync(src, dst);
      }
    }
  }
  const catalogVersion = Number(readWebCatalogVersion()) || 1;
  const codes = ["en", "es"];
  for (const code of codes) {
    const tomlPath = path.join(ROOT, "sow-i18n", "strings", code, "web.toml");
    if (!fs.existsSync(tomlPath)) continue;
    const parsed = Bun.TOML.parse(fs.readFileSync(tomlPath, "utf8")) as Record<string, unknown>;
    const baseSrc = path.join(distLocales, code);
    const outPath = path.join(LOCALES_OUT, code);
    let base: Record<string, unknown> = {};
    if (fs.existsSync(baseSrc)) {
      try {
        base = JSON.parse(fs.readFileSync(baseSrc, "utf8")) as Record<string, unknown>;
      } catch {
        base = {};
      }
    }
    const existingStrings =
      base.strings && typeof base.strings === "object" && !Array.isArray(base.strings)
        ? (base.strings as Record<string, unknown>)
        : {};
    const merged: Record<string, unknown> = {
      schema: base.schema ?? 1,
      version: base.version ?? catalogVersion,
      locale: code,
      languages: base.languages ?? [
        { code: "en", name: "English" },
        { code: "es", name: "Español" },
      ],
      strings: deepMergeStrings(existingStrings, parsed),
    };
    fs.writeFileSync(outPath, JSON.stringify(merged));
  }
  void liveAssetsDir;
  const available = fs.readdirSync(LOCALES_OUT).filter((f) => /^[a-z]{2}(-[a-z]{2})?$/.test(f));
  return available.length ? available.sort() : ["en", "es"];
}

async function buildShell(
  wasmInfo: { jsFile: string; wasmFile: string },
  liveAssetsDir: string,
  localeCodes: string[],
) {
  const t0 = performance.now();
  fs.mkdirSync(PLAY_DIR, { recursive: true });
  fs.mkdirSync(SDK_OUT, { recursive: true });

  const [loaderJs, expEventsJs, menuBundleJs, storePortalsJs] = await Promise.all([
    compileTs("loader.ts"),
    compileTs("experience_events.ts"),
    compileTs("main_menu.ts"),
    compileTs("sdk/store_portals.ts"),
  ]);

  fs.writeFileSync(path.join(SDK_OUT, "store_portals.js"), storePortalsJs);
  for (const assetName of ["sow.svg", "icon-192.png", "favicon.ico", "game-manifest.json"]) {
    const fromDist = path.join(ROOT, "dist", "web", assetName);
    const fromShell = path.join(ROOT, "assets", "art", "brand", assetName);
    const src = fs.existsSync(fromDist) ? fromDist : fromShell;
    if (fs.existsSync(src)) fs.copyFileSync(src, path.join(OUT_DIR, assetName));
  }
  const controlsCss = path.join(SHELL_DIR, "sow-controls.css");
  if (fs.existsSync(controlsCss)) fs.copyFileSync(controlsCss, path.join(OUT_DIR, "sow-controls.css"));

  const buildingAtlasCandidates = [
    path.join(liveAssetsDir, "gameplay", "buildings", "atlas.json"),
    path.join(ROOT, "assets", "generated", "buildings-atlas.json"),
  ];
  let buildingManifest: unknown = null;
  for (const candidate of buildingAtlasCandidates) {
    if (fs.existsSync(candidate)) {
      buildingManifest = JSON.parse(fs.readFileSync(candidate, "utf8"));
      break;
    }
  }

  const menuJs =
    `window.SOW_BUILDING_ART = ${JSON.stringify(buildingManifest)};\n` +
    menuBundleJs.replaceAll("</script>", "<\\/script>");
  const menuHash = crypto.createHash("sha256").update(menuJs).digest("hex").slice(0, 8);
  const menuFilename = `main_menu_${menuHash}.js`;
  for (const old of fs.readdirSync(PLAY_DIR)) {
    if (old.startsWith("main_menu_") && old.endsWith(".js") && old !== menuFilename) {
      fs.unlinkSync(path.join(PLAY_DIR, old));
    }
  }
  fs.writeFileSync(path.join(PLAY_DIR, menuFilename), menuJs);

  const menuCss = CSS_FILES.map((name) => fs.readFileSync(path.join(SHELL_DIR, name), "utf8")).join("\n");
  const ts = String(Math.floor(Date.now() / 1000));
  const catalogVersion = readWebCatalogVersion();

  const ptrHost = process.env.SOW_DEV_HOST || "ptr.shadowsofwar.io";
  const portalBootJs = [
    `window.SOW_WS_URL = "wss://${ptrHost}/ws/";`,
    `window.SOW_MAPS_URL = "/maps";`,
    `window.SOW_ASSETS_URL = "/assets";`,
    `window.SOW_DATABASE_URL = "https://${ptrHost}/api";`,
    `window.SOW_SOCIAL_SIGN_IN = false;`,
  ].join(" ");

  let html = fs.readFileSync(path.join(SHELL_DIR, "index.html.template"), "utf8");
  html = html
    .replaceAll("<!-- __SOW_SOURCE_IDENTITY_SLOT__ -->", "")
    .replaceAll("<!-- __SOW_EXPERIENCE_EVENTS_SLOT__ -->", "")
    .replaceAll("<!-- __SOW_ANALYTICS_SCRIPT_SLOT__ -->", "")
    .replaceAll("<!-- __SOW_SERVICE_WORKER_SLOT__ -->", "")
    .replaceAll(
      "/* PORTAL_BOOT_SLOT: SOW_PORTAL / SOW_WS_URL overrides injected by sow-dist crazygames. */",
      portalBootJs,
    )
    .replaceAll("__VERSION__", "dev")
    .replaceAll("__SITE_ORIGIN__", "sow://app")
    .replaceAll("./__JS_FILE__", "../__JS_FILE__")
    .replaceAll("./__WASM_FILE__", "../__WASM_FILE__")
    .replaceAll("__JS_FILE__", wasmInfo.jsFile)
    .replaceAll("__WASM_FILE__", wasmInfo.wasmFile)
    .replaceAll("__BUILD_TS__", ts)
    .replaceAll("__SOW_LOCALE_BASE__", "../locales")
    .replaceAll("__SOW_LOCALE_CATALOG_VERSION__", catalogVersion)
    .replaceAll("__SOW_LOCALE_CODES__", JSON.stringify(localeCodes))
    .replaceAll("__MAPS_CACHE_BUST__", ts)
    .replaceAll("__ASSETS_UI_BASE__", "/assets/shell/loader/")
    .replaceAll('href="./sow.svg"', 'href="../sow.svg"')
    .replaceAll('content="./icon-192.png"', 'content="../icon-192.png"')
    .replaceAll('href="./favicon.ico"', 'href="../favicon.ico"')
    .replaceAll('src="./loader.js"', 'src="../loader.js"')
    .replaceAll(`src="./sdk/store_portals.js?v=${ts}"`, `src="../sdk/store_portals.js?v=${ts}"`)
    .replace("/* __INLINE_EXPERIENCE_EVENTS_JS__ */", () => expEventsJs.replaceAll("</script>", "<\\/script>"))
    .replace("/* __INLINE_LOADER_JS__ */", () => `window.SOW_ASSET_TS = {};\n${loaderJs.replaceAll("</script>", "<\\/script>")}`)
    .replace("/* __INLINE_MAIN_MENU_CSS__ */", () => menuCss)
    .replace(
      "<script>/* __INLINE_MAIN_MENU_JS__ */</script>",
      () => `<link rel="preload" href="./${menuFilename}" as="script">\n    <script src="./${menuFilename}"></script>`,
    );

  fs.writeFileSync(path.join(PLAY_DIR, "index.html"), html);
  console.log(`✅ Shell rebuilt in ${Math.round(performance.now() - t0)} ms`);
}

function touchReloadStamp() {
  fs.writeFileSync(path.join(OUT_DIR, ".reload-stamp"), String(Date.now()));
}

function ensureDevHeroAvatars(liveAssetsDir: string): void {
  const heroesDir = path.join(ROOT, "assets", "art", "avatars", "heroes");
  if (!fs.existsSync(heroesDir)) return;
  const devAvatarsDir = path.join(OUT_DIR, "assets", "gameplay", "avatars");
  const devExprDir = path.join(devAvatarsDir, "expressions");
  const missing: { slug: string; src: string }[] = [];
  for (const file of fs.readdirSync(heroesDir)) {
    if (!file.endsWith(".webp")) continue;
    const slug = file.slice(0, -".webp".length);
    const liveCalm = path.join(liveAssetsDir, "gameplay", "avatars", `${slug}.webp`);
    const devCalm = path.join(devAvatarsDir, `${slug}.webp`);
    const srcPath = path.join(heroesDir, file);
    if (fs.existsSync(liveCalm) && fs.statSync(srcPath).mtimeMs <= fs.statSync(liveCalm).mtimeMs) {
      continue;
    }
    if (fs.existsSync(devCalm) && fs.statSync(srcPath).mtimeMs <= fs.statSync(devCalm).mtimeMs) {
      continue;
    }
    missing.push({ slug, src: srcPath });
  }
  if (!missing.length) return;
  fs.mkdirSync(devAvatarsDir, { recursive: true });
  fs.mkdirSync(devExprDir, { recursive: true });
  const py = `
import os, sys, json
from PIL import Image
items = json.loads(sys.argv[1])
avatars_dir, expr_dir = sys.argv[2], sys.argv[3]
exprs = [("happy", 0, 0), ("calm", 1, 0), ("fear", 0, 1), ("angry", 1, 1)]
for item in items:
    im = Image.open(item["src"]).convert("RGBA")
    w, h = im.size
    hw, hh = w // 2, h // 2
    inset = max(1, round(min(hw, hh) * 0.01))
    for name, cx, cy in exprs:
        box = (cx * hw + inset, cy * hh + inset, (cx + 1) * hw - inset, (cy + 1) * hh - inset)
        q = im.crop(box)
        s = min(q.size)
        q = q.crop(((q.width - s) // 2, (q.height - s) // 2, (q.width + s) // 2, (q.height + s) // 2))
        q = q.resize((192, 192), Image.Resampling.LANCZOS)
        out = os.path.join(avatars_dir, f"{item['slug']}.webp") if name == "calm" else os.path.join(expr_dir, f"{item['slug']}_{name}.webp")
        q.save(out, "WEBP", quality=88)
`;
  const res = spawnSync("python3", ["-c", py, JSON.stringify(missing), devAvatarsDir, devExprDir], {
    stdio: "inherit",
  });
  if (res.status === 0) {
    console.log(`✅ Staged ${missing.length} dev hero avatar set(s)`);
  }
}

async function main() {
  const liveAssetsDir = findLiveAssetsDir();
  ensureDevHeroAvatars(liveAssetsDir);
  let wasmInfo = ensureWasm(forceWasm);
  let localeCodes = buildLocales(liveAssetsDir);
  await buildShell(wasmInfo, liveAssetsDir, localeCodes);
  if (dryRun) {
    console.log(`Dry run OK: ${PLAY_DIR}/index.html (WASM: ${wasmInfo.wasmFile})`);
    return;
  }
  touchReloadStamp();

  const electronBin = resolveElectronBin();
  console.log(`==> Launching Electron 44 (${electronBin})`);
  console.log("    Hotkeys: F5/Ctrl+R reload · F9 Desktop/Mobile toggle · F12 DevTools · Ctrl+Shift+R reset intro");

  const electronArgs = [path.join(ROOT, "sow-native", "main.js"), OUT_DIR];
  if (startMobile) electronArgs.push("--mobile");

  const child = spawn(electronBin, electronArgs, {
    cwd: path.join(ROOT, "sow-native"),
    stdio: "inherit",
    env: {
      ...process.env,
      ELECTRON_FORCE_IS_PACKAGED: "0",
      ELECTRON_IS_DEV: "1",
      ELECTRON_DISABLE_SECURITY_WARNINGS: "true",
      SOW_WEBROOT: OUT_DIR,
      SOW_LIVE_ROOT: ROOT,
      SOW_LIVE_ASSETS: liveAssetsDir,
    },
  });

  let rebuildTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingShell = false;
  let pendingLocales = false;
  let pendingRust = false;
  let building = false;

  const scheduleRebuild = () => {
    if (rebuildTimer) clearTimeout(rebuildTimer);
    rebuildTimer = setTimeout(async () => {
      rebuildTimer = null;
      if (building) {
        scheduleRebuild();
        return;
      }
      building = true;
      const doRust = pendingRust;
      const doLocales = pendingLocales;
      const doShell = pendingShell;
      pendingRust = false;
      pendingLocales = false;
      pendingShell = false;
      try {
        if (doRust) wasmInfo = ensureWasm(true);
        if (doLocales) localeCodes = buildLocales(liveAssetsDir);
        if (doShell || doLocales) await buildShell(wasmInfo, liveAssetsDir, localeCodes);
        touchReloadStamp();
      } catch (err) {
        console.error("⚠️ Dev rebuild failed:", err);
      } finally {
        building = false;
        if (pendingShell || pendingLocales || pendingRust) scheduleRebuild();
      }
    }, 120);
  };

  const watchers: fs.FSWatcher[] = [];
  watchers.push(
    fs.watch(SHELL_DIR, { recursive: true }, (_ev, filename) => {
      if (!filename || filename.includes(".sow-menu-")) return;
      pendingShell = true;
      scheduleRebuild();
    }),
  );
  watchers.push(
    fs.watch(path.join(ROOT, "assets", "campaign"), { recursive: true }, () => {
      scheduleRebuild();
    }),
  );
  watchers.push(
    fs.watch(path.join(ROOT, "sow-i18n", "strings"), { recursive: true }, () => {
      pendingLocales = true;
      scheduleRebuild();
    }),
  );
  if (watchRust) {
    for (const crate of ["sow-client", "sow-core", "sow-data", "sow-render"]) {
      watchers.push(
        fs.watch(path.join(ROOT, crate, "src"), { recursive: true }, () => {
          pendingRust = true;
          scheduleRebuild();
        }),
      );
    }
  }

  child.on("exit", (code) => {
    for (const w of watchers) w.close();
    process.exit(code ?? 0);
  });
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
