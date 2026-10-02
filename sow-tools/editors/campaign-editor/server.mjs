import http from "node:http";
import { createHash, randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { promises as fs } from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const editorDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(editorDir, "../../..");
const campaignDir = path.join(root, "assets/campaign");
const mapsDir = path.join(root, "assets/maps");
const entityCatalogFile = path.join(root, "assets/geo_entities.json");
const distLocaleDir = path.join(root, "dist/web/locales");
const campaignRequire = createRequire(import.meta.url);
const campaignFile = path.join(root, "sow-web/shell/sow-campaign.js");
const maxBodyBytes = 1024 * 1024;
const saving = new Set();
let entityCatalogSaving = false;
let mapRosterSaving = false;
const episodeFile = /^[a-z][a-z0-9_]{0,63}(?:\.triggers)?\.json$/;
const entityId = /^[a-z][a-z0-9_]{0,95}$/;
const avatarId = /^[a-z][a-z0-9_]*$/;
const mapId = /^[a-z][a-z0-9_]{0,63}$/;
const sharedShell = new Set(["sow-controls.css", "main_menu.tutorial.css", "sow-campaign.js", "sow-campaign-view.js"]);

function loadCampaignRuntime() {
  const modulePath = campaignRequire.resolve(campaignFile);
  delete campaignRequire.cache[modulePath];
  return campaignRequire(modulePath);
}

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
  if (pathname === "/tools/campaign-editor/atlas") return path.join(editorDir, "atlas.html");
  if (pathname === "/tools/campaign-editor/map-rosters") return path.join(editorDir, "map-rosters.html");
  if (pathname === "/" || pathname === "/tools/campaign-editor" || pathname === "/tools/campaign-editor/") {
    return path.join(editorDir, "index.html");
  }
  if (pathname.startsWith("/tools/campaign-editor/")) return safeFile(editorDir, pathname.slice("/tools/campaign-editor/".length));
  if (pathname.startsWith("/assets/campaign/")) return safeFile(campaignDir, pathname.slice("/assets/campaign/".length));
  if (pathname.startsWith("/assets/maps/")) return safeFile(path.join(root, "assets/maps"), pathname.slice("/assets/maps/".length));
  if (pathname.startsWith("/assets/gameplay/")) return safeFile(path.join(root, "assets/gameplay"), pathname.slice("/assets/gameplay/".length));
  if (pathname.startsWith("/assets/map_sources/")) return safeFile(path.join(root, "assets/map_sources"), pathname.slice("/assets/map_sources/".length));
  if (pathname.startsWith("/fonts/")) return safeFile(path.join(root, "sow-web/site/fonts"), pathname.slice("/fonts/".length));
  if (pathname.startsWith("/locales/")) return safeFile(distLocaleDir, pathname.slice("/locales/".length));
  if (pathname.startsWith("/shell/")) {
    const name = pathname.slice("/shell/".length);
    if (sharedShell.has(name)) return path.join(root, "sow-web/shell", name);
  }
  return null;
}

async function readBody(req, limit = maxBodyBytes) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > limit) throw new Error("request body too large");
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

function saveMapRosters(mapId, body) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.env.CARGO || "cargo", [
      "run", "--quiet", "-p", "sow-tools", "--", "save-map-rosters",
      "--map", mapId, "--maps-root", mapsDir
    ], { cwd: root, stdio: ["pipe", "ignore", "pipe"] });
    let stderr = "";
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", chunk => { stderr += chunk; });
    child.on("error", reject);
    child.on("close", code => code === 0 ? resolve() : reject(Object.assign(new Error(stderr.trim() || `sow-tools exited ${code}`), { invalidRoster: stderr.includes("sow-tools: command failed:") })));
    child.stdin.end(body);
  });
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
  const names = new Set(), ids = new Set();
  for (const faction of roster.factions) {
    if (!faction || typeof faction.id !== "string" || !entityId.test(faction.id) || ids.has(faction.id)
      || typeof faction.name !== "string" || !faction.name.trim() || names.has(faction.name)
      || !Number.isInteger(faction.starting_troops) || faction.starting_troops < 0 || faction.starting_troops > 1000000
      || !Number.isFinite(faction.x) || !Number.isFinite(faction.y)) throw new Error("Invalid or repeated faction in map.");
    ids.add(faction.id); names.add(faction.name);
  }
  const avatarFiles = new Set((await fs.readdir(path.join(root, "assets/gameplay/avatars")))
    .filter(name => /^[a-z][a-z0-9_]*\.webp$/.test(name)).map(name => name.slice(0, -5)));
  const result = loadCampaignRuntime().validate(definition, roster, {
    hasText: key => hasText(key, definition),
    hasAvatar: avatar => avatarFiles.has(avatar),
  });
  if (definition.episode_id !== episodeId) throw new Error("Episode ID must match its file name.");
  if (result.errors.length) throw new Error(result.errors.map(issue => [issue.step, issue.field, issue.message].filter(Boolean).join(" · ")).join("\n"));
}

async function validateEntityCatalog(value) {
  if (!value || value.version !== 1 || !Array.isArray(value.entities) || value.entities.length > 10000) throw new Error("Invalid entity catalog.");
  const ids = new Set();
  const names = new Set();
  const avatarFiles = new Set((await fs.readdir(path.join(root, "assets/gameplay/avatars")))
    .filter(name => /^[a-z][a-z0-9_]*\.webp$/.test(name)).map(name => name.slice(0, -5)));
  const kinds = new Set(["tribe", "city_state", "kingdom", "empire", "country", "state_region"]);
  const eras = new Set(["unspecified", "ancient", "classical", "medieval", "early_modern", "modern"]);
  const regions = new Set(["unassigned", "europe", "africa", "asia", "americas", "oceania"]);
  const poolOrders = [new Set(), new Set()];
  for (const entity of value.entities) {
    if (!entity || typeof entity.id !== "string" || !entityId.test(entity.id) || ids.has(entity.id)
      || typeof entity.name !== "string" || !entity.name.trim() || entity.name.length > 120
      || !kinds.has(entity.kind) || !eras.has(entity.era) || !regions.has(entity.region)
      || typeof entity.flag !== "string" || (entity.flag && !/^[a-z]{2}$/.test(entity.flag))
      || (entity.territory_color != null && (typeof entity.territory_color !== "string" || !/^#[0-9a-fA-F]{6}$/.test(entity.territory_color)))
      || !Array.isArray(entity.maps) || entity.maps.some(id => typeof id !== "string" || !mapId.test(id))) throw new Error("Invalid or duplicate entity.");
    ids.add(entity.id);
    const normalizedName = entity.name.trim().toLocaleLowerCase();
    if (names.has(normalizedName)) throw new Error(`Duplicate entity name: ${entity.name}`);
    names.add(normalizedName);
    if ((entity.lat == null) !== (entity.lon == null)
      || (entity.lat != null && (!Number.isFinite(entity.lat) || entity.lat < -90 || entity.lat > 90
        || !Number.isFinite(entity.lon) || entity.lon < -180 || entity.lon > 180))) throw new Error(`Invalid coordinates: ${entity.id}`);
    if (entity.avatar != null && (typeof entity.avatar !== "string" || !avatarId.test(entity.avatar) || !avatarFiles.has(entity.avatar))) throw new Error(`Missing avatar: ${entity.id}`);
    for (const [field, used] of [["fallback_nation_order", poolOrders[0]], ["fallback_tribe_order", poolOrders[1]]]) {
      const order = entity[field];
      if (order != null && (!Number.isSafeInteger(order) || order < 0 || used.has(order))) throw new Error(`Invalid ${field}: ${entity.id}`);
      if (order != null) used.add(order);
    }
  }
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

  if (req.method === "GET" && url.pathname === "/__entities") {
    const body = await fs.readFile(entityCatalogFile);
    const tag = etag(body);
    reply(res, 200, body, "application/json; charset=utf-8", { ETag: tag });
    return;
  }
  if (req.method === "POST" && url.pathname === "/__entities") {
    if (entityCatalogSaving) { reply(res, 409, "An entity catalog save is already in progress."); return; }
    entityCatalogSaving = true;
    try {
      let body;
      try { body = await readBody(req, 8 * 1024 * 1024); }
      catch (error) { reply(res, 413, error.message || "request body too large"); return; }
      let value;
      try { value = JSON.parse(body.toString("utf8")); }
      catch (error) { reply(res, 400, error.message || "invalid JSON"); return; }
      const current = await fs.readFile(entityCatalogFile);
      if (!req.headers["if-match"] || req.headers["if-match"] !== etag(current)) {
        reply(res, 409, "The entity catalog changed since it was loaded. Reload before saving.");
        return;
      }
      try { await validateEntityCatalog(value); }
      catch (error) { reply(res, 400, error.message || "invalid entity catalog"); return; }
      const temp = path.join(editorDir, ".entity-catalog-save-" + randomUUID() + ".tmp");
      try {
        await fs.writeFile(temp, JSON.stringify(value, null, 2) + "\n", { flag: "wx" });
        await fs.rename(temp, entityCatalogFile);
      } finally {
        await fs.rm(temp, { force: true });
      }
      const saved = await fs.readFile(entityCatalogFile);
      console.log("  saved assets/geo_entities.json");
      reply(res, 200, JSON.stringify({ saved: "assets/geo_entities.json" }), "application/json; charset=utf-8", { ETag: etag(saved) });
    } catch (error) {
      if (error.code === "ENOENT") reply(res, 404, "entity catalog does not exist");
      else throw error;
    } finally {
      entityCatalogSaving = false;
    }
    return;
  }
  if (req.method === "GET" && url.pathname === "/__entity_maps") {
    const entries = await fs.readdir(path.join(root, "assets/maps"), { withFileTypes: true });
    const maps = entries.filter(entry => entry.isDirectory() && mapId.test(entry.name)
      && (existsSync(path.join(root, "assets/maps", entry.name, "map.bin"))
        || existsSync(path.join(root, "assets/maps", entry.name, "map.bin.br"))))
      .map(entry => entry.name).sort();
    reply(res, 200, JSON.stringify(maps), "application/json; charset=utf-8");
    return;
  }

  if (url.pathname === "/__map-rosters" && req.method === "POST") {
    const mapId = url.searchParams.get("map") || "";
    if (!mapId || !/^[a-z][a-z0-9_]{0,63}$/.test(mapId)) { reply(res, 400, "Invalid map key."); return; }
    if (mapRosterSaving) { reply(res, 409, "A map roster save is already running."); return; }
    mapRosterSaving = true;
    try {
      let body;
      try { body = await readBody(req, 8 * 1024 * 1024); }
      catch (error) { reply(res, 413, error.message || "request body too large"); return; }
      const mapFile = path.join(mapsDir, mapId, "map.bin");
      const current = await fs.readFile(mapFile);
      if (!req.headers["if-match"] || req.headers["if-match"] !== etag(current)) {
        reply(res, 409, "This map changed since it was loaded. Reload before saving.");
        return;
      }
      try { await saveMapRosters(mapId, body); }
      catch (error) {
        reply(res, error.invalidRoster ? 400 : 500, error.message || "Could not save map roster.");
        return;
      }
      const saved = await fs.readFile(mapFile);
      console.log(`  saved assets/maps/${mapId}/map.bin roster`);
      reply(res, 200, JSON.stringify({ saved: `assets/maps/${mapId}/map.bin` }), "application/json; charset=utf-8", { ETag: etag(saved) });
    } catch (error) {
      if (error.code === "ENOENT") reply(res, 404, "Map file does not exist.");
      else throw error;
    } finally {
      mapRosterSaving = false;
    }
    return;
  }

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
    const isTriggerFile = file.endsWith(".triggers.json");
    const factionRenames = !isTriggerFile && Array.isArray(value && value.faction_renames) ? value.faction_renames : null;
    if (factionRenames) {
      value = value.roster;
      body = Buffer.from(JSON.stringify(value, null, 2) + "\n");
    }
    if (saving.has(episodeId)) { reply(res, 409, "A save for this episode is already in progress."); return; }
    saving.add(episodeId);
    let bodyTag;
    try {
      const current = await fs.readFile(target);
      if (!req.headers["if-match"] || req.headers["if-match"] !== etag(current)) {
        reply(res, 409, "This episode changed since it was loaded. Reload before saving.");
        return;
      }
      if (factionRenames) {
        const previousNames = new Set((JSON.parse(current.toString("utf8")).factions || []).map(faction => faction.name));
        const nextNames = new Set((value.factions || []).map(faction => faction.name));
        const renamedFrom = new Set();
        if (!factionRenames.length || factionRenames.some(item => {
          if (!item || typeof item.from !== "string" || !item.from || typeof item.to !== "string" || !item.to
            || item.from === item.to || !previousNames.has(item.from) || !nextNames.has(item.to) || renamedFrom.has(item.from)) return true;
          renamedFrom.add(item.from);
          return false;
        })) {
          reply(res, 400, "Invalid faction rename list."); return;
        }
        const definitionPath = path.join(campaignDir, episodeId + ".triggers.json");
        const definition = loadCampaignRuntime().renameFactionText(
          JSON.parse(await fs.readFile(definitionPath, "utf8")), factionRenames
        );
        try { await validatePair(episodeId, value, definition); }
        catch (error) { reply(res, 400, error.message || "invalid campaign data"); return; }
        const definitionBody = Buffer.from(JSON.stringify(definition, null, 2) + "\n");
        const definitionTemp = path.join(editorDir, ".campaign-save-" + randomUUID() + ".tmp");
        const rosterTemp = path.join(editorDir, ".campaign-save-" + randomUUID() + ".tmp");
        try {
          await fs.writeFile(definitionTemp, definitionBody, { flag: "wx" });
          await fs.writeFile(rosterTemp, body, { flag: "wx" });
          await fs.rename(definitionTemp, definitionPath);
          await fs.rename(rosterTemp, target);
        } finally {
          await Promise.all([fs.rm(definitionTemp, { force: true }), fs.rm(rosterTemp, { force: true })]);
        }
      } else {
        try { await validateSave(file, value); }
        catch (error) { reply(res, 400, error.message || "invalid campaign data"); return; }
        const temp = path.join(editorDir, ".campaign-save-" + randomUUID() + ".tmp");
        try {
          await fs.writeFile(temp, body, { flag: "wx" });
          await fs.rename(temp, target);
        } finally {
          await fs.rm(temp, { force: true });
        }
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
  console.log(`Entity Atlas: http://127.0.0.1:${port}/tools/campaign-editor/atlas`);
  console.log(`Map Rosters: http://127.0.0.1:${port}/tools/campaign-editor/map-rosters`);
  console.log("  Saving in the editor updates assets/campaign directly; refresh the local game to use them.");
  console.log("  Ctrl-C to stop.");
});
