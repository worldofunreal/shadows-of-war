import http from "node:http";
import { createHash, randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import { promises as fs } from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const editorDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(editorDir, "../../..");
const campaignDir = path.join(root, "assets/campaign");
const distLocaleDir = path.join(root, "dist/web/locales");
const campaign = createRequire(import.meta.url)(path.join(root, "sow-web/shell/sow-campaign.js"));
const maxBodyBytes = 1024 * 1024;
const saving = new Set();
const episodeFile = /^[a-z][a-z0-9_]{0,63}(?:\.triggers)?\.json$/;
const sharedShell = new Set(["sow-controls.css", "main_menu.tutorial.css", "sow-campaign.js", "sow-campaign-view.js"]);

function portFromArgs() {
  const args = process.argv.slice(2);
  const flag = args.findIndex(arg => arg === "--port" || arg === "-p");
  const port = Number(flag >= 0 ? args[flag + 1] : process.env.SOW_EDITOR_PORT || "8777");
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error("invalid editor port");
  return port;
}

function reply(res, status, body, type = "text/plain; charset=utf-8", headers = {}) {
  res.writeHead(status, { "Content-Type": type, "Cache-Control": "no-store", ...headers });
  res.end(body);
}

function contentType(file) {
  return ({
    ".html": "text/html; charset=utf-8", ".json": "application/json; charset=utf-8",
    ".js": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8",
    ".bin": "application/octet-stream", ".br": "application/octet-stream",
    ".webp": "image/webp", ".png": "image/png", ".ttf": "font/ttf", ".ttc": "font/collection",
    ".woff2": "font/woff2", ".svg": "image/svg+xml"
  })[path.extname(file)] || "application/octet-stream";
}

function safeFile(base, relative) {
  const decoded = decodeURIComponent(relative);
  if (decoded.includes("\\") || decoded.split("/").some(part => part === "..")) return null;
  const file = path.resolve(base, decoded);
  return file === base || file.startsWith(base + path.sep) ? file : null;
}

function staticFile(pathname) {
  if (pathname === "/" || pathname === "/tools/campaign-editor" || pathname === "/tools/campaign-editor/") {
    return path.join(editorDir, "index.html");
  }
  if (pathname.startsWith("/tools/campaign-editor/")) return safeFile(editorDir, pathname.slice("/tools/campaign-editor/".length));
  if (pathname.startsWith("/assets/campaign/")) return safeFile(campaignDir, pathname.slice("/assets/campaign/".length));
  if (pathname.startsWith("/assets/maps/")) return safeFile(path.join(root, "assets/maps"), pathname.slice("/assets/maps/".length));
  if (pathname.startsWith("/assets/gameplay/")) return safeFile(path.join(root, "assets/gameplay"), pathname.slice("/assets/gameplay/".length));
  if (pathname.startsWith("/fonts/")) return safeFile(path.join(root, "sow-web/site/fonts"), pathname.slice("/fonts/".length));
  if (pathname.startsWith("/locales/")) return safeFile(distLocaleDir, pathname.slice("/locales/".length));
  if (pathname.startsWith("/shell/")) {
    const name = pathname.slice("/shell/".length);
    if (sharedShell.has(name)) return path.join(root, "sow-web/shell", name);
  }
  return null;
}

async function readBody(req) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > maxBodyBytes) throw new Error("request body too large");
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

function etag(data) {
  return '"' + createHash("sha256").update(data).digest("hex") + '"';
}

function hasText(key, definition) {
  const custom = definition.strings && definition.strings[definition.default_locale];
  if (custom && typeof custom[key] === "string" && custom[key].trim()) return true;
  const catalogFile = path.join(distLocaleDir, definition.default_locale || "en");
  try {
    const catalog = JSON.parse(readFileSync(catalogFile, "utf8")).strings;
    const value = key.split(".").reduce((node, part) => node && node[part], catalog);
    return typeof value === "string" && value.trim().length > 0;
  } catch {
    return false;
  }
}

async function validatePair(episodeId, roster, definition) {
  if (!roster || typeof roster.map !== "string" || !Array.isArray(roster.player_spawn) || roster.player_spawn.length !== 2 || !roster.player_spawn.every(Number.isFinite) || !Array.isArray(roster.factions) || !roster.factions.length) throw new Error("Invalid campaign map or factions.");
  const roles = new Set(["kin", "independent", "vassal", "boss", "big_boss", "neutral"]);
  const names = new Set();
  for (const faction of roster.factions) {
    if (!faction || typeof faction.name !== "string" || !faction.name.trim() || names.has(faction.name) || !roles.has(faction.role) || !Number.isFinite(faction.x) || !Number.isFinite(faction.y)) throw new Error("Invalid or repeated faction in map.");
    names.add(faction.name);
  }
  const avatarFiles = new Set((await fs.readdir(path.join(root, "assets/gameplay/avatars")))
    .filter(name => /^[a-z][a-z0-9_]*\.webp$/.test(name)).map(name => name.slice(0, -5)));
  const result = campaign.validate(definition, roster, {
    hasText: key => hasText(key, definition),
    hasAvatar: avatar => avatarFiles.has(avatar)
  });
  if (definition.episode_id !== episodeId) throw new Error("Episode ID must match its file name.");
  if (result.errors.length) throw new Error(result.errors.map(issue => [issue.step, issue.field, issue.message].filter(Boolean).join(" · ")).join("\n"));
}

async function validateSave(file, value) {
  if (!episodeFile.test(file) || file.includes("..")) throw new Error("Invalid campaign file.");
  const trigger = file.endsWith(".triggers.json");
  const episodeId = file.replace(/(?:\.triggers)?\.json$/, "");
  const companion = trigger ? JSON.parse(await fs.readFile(path.join(campaignDir, episodeId + ".json"), "utf8")) : value;
  const definition = trigger ? value : JSON.parse(await fs.readFile(path.join(campaignDir, episodeId + ".triggers.json"), "utf8"));
  await validatePair(episodeId, companion, definition);
}

async function handle(req, res) {
  let url;
  try { url = new URL(req.url, "http://127.0.0.1"); }
  catch { reply(res, 400, "bad URL"); return; }

  if (req.method === "POST" && url.pathname === "/__save") {
    const file = url.searchParams.get("file") || "";
    if (!episodeFile.test(file) || file.includes("..")) { reply(res, 400, "Invalid campaign file."); return; }
    const episodeId = file.replace(/(?:\.triggers)?\.json$/, "");
    let body, value;
    try {
      body = await readBody(req);
      value = JSON.parse(body.toString("utf8"));
    } catch (error) {
      reply(res, 400, error.message || "invalid JSON");
      return;
    }
    const target = path.join(campaignDir, file);
    if (saving.has(episodeId)) { reply(res, 409, "A save for this episode is already in progress."); return; }
    saving.add(episodeId);
    let bodyTag;
    try {
      const current = await fs.readFile(target);
      if (!req.headers["if-match"] || req.headers["if-match"] !== etag(current)) {
        reply(res, 409, "This episode changed since it was loaded. Reload before saving.");
        return;
      }
      try { await validateSave(file, value); }
      catch (error) { reply(res, 400, error.message || "invalid campaign data"); return; }
      const temp = path.join(editorDir, ".campaign-save-" + randomUUID() + ".tmp");
      try {
        await fs.writeFile(temp, body, { flag: "wx" });
        await fs.rename(temp, target);
      } finally {
        await fs.rm(temp, { force: true });
      }
      bodyTag = etag(body);
    } catch (error) {
      if (error.code === "ENOENT") { reply(res, 404, "campaign file does not exist"); return; }
      throw error;
    } finally {
      saving.delete(episodeId);
    }
    if (!bodyTag) {
      return;
    }
    console.log(`  saved assets/campaign/${file}`);
    reply(res, 200, JSON.stringify({ saved: file }), "application/json; charset=utf-8", { ETag: bodyTag });
    return;
  }
  if (req.method === "GET" && url.pathname === "/__avatars") {
    const avatars = (await fs.readdir(path.join(root, "assets/gameplay/avatars")))
      .filter(name => /^[a-z][a-z0-9_]*\.webp$/.test(name))
      .map(name => name.slice(0, -5)).sort();
    reply(res, 200, JSON.stringify(avatars), "application/json; charset=utf-8");
    return;
  }
  if (req.method === "GET" && url.pathname === "/__episodes") {
    const files = new Set(await fs.readdir(campaignDir));
    const episodes = [...files]
      .filter(name => name.endsWith(".triggers.json"))
      .map(name => name.slice(0, -".triggers.json".length))
      .filter(id => /^[a-z][a-z0-9_]{0,63}$/.test(id) && files.has(id + ".json"))
      .sort();
    reply(res, 200, JSON.stringify(episodes), "application/json; charset=utf-8");
    return;
  }
  if (req.method !== "GET" && req.method !== "HEAD") { reply(res, 405, "method not allowed"); return; }

  let file;
  try { file = staticFile(url.pathname); }
  catch { reply(res, 400, "bad path"); return; }
  if (!file) { reply(res, 404, "not found"); return; }
  try {
    const body = await fs.readFile(file);
    const tag = etag(body);
    if (req.headers["if-none-match"] === tag) { res.writeHead(304, { ETag: tag, "Cache-Control": "no-store" }); res.end(); return; }
    res.writeHead(200, { "Content-Type": contentType(file), "Cache-Control": "no-store", ETag: tag });
    res.end(req.method === "HEAD" ? undefined : body);
  } catch { reply(res, 404, "not found"); }
}

const port = portFromArgs();
const server = http.createServer((req, res) => {
  handle(req, res).catch(error => {
    console.error(error);
    if (!res.headersSent) reply(res, 500, "editor server error");
  });
});

server.listen(port, "127.0.0.1", () => {
  console.log(`Campaign editor: http://127.0.0.1:${port}/tools/campaign-editor/`);
  console.log("  Map and story export update assets/campaign directly; refresh the local game to use them.");
  console.log("  Ctrl-C to stop.");
});
