import http from "node:http";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const editorDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(editorDir, "../../..");
const campaignDir = path.join(root, "assets/campaign");
const maxBodyBytes = 1024 * 1024;
const roles = new Set(["kin", "independent", "vassal", "boss", "big_boss", "neutral"]);
const triggerTypes = new Set(["territory", "kills", "defeated", "contact", "attack", "troops", "building", "fleet", "nuke", "elapsed"]);
const actionTypes = new Set(["show_dialog", "set_objective", "emote", "pause", "resume", "set_flag"]);

function portFromArgs() {
  const args = process.argv.slice(2);
  const flag = args.findIndex(arg => arg === "--port" || arg === "-p");
  const value = flag >= 0 ? args[flag + 1] : process.env.SOW_EDITOR_PORT || "8777";
  const port = Number(value);
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error("invalid editor port");
  return port;
}

function reply(res, status, body, type = "text/plain; charset=utf-8") {
  res.writeHead(status, { "Content-Type": type, "Cache-Control": "no-store" });
  res.end(body);
}

function contentType(file) {
  return {
    ".html": "text/html; charset=utf-8",
    ".json": "application/json; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".css": "text/css; charset=utf-8",
    ".bin": "application/octet-stream"
  }[path.extname(file)] || "application/octet-stream";
}

function staticFile(urlPath) {
  const decoded = decodeURIComponent(urlPath);
  let relative = decoded.slice(1);
  if (decoded === "/" || decoded === "/tools/campaign-editor" || decoded === "/tools/campaign-editor/") {
    relative = "sow-tools/editors/campaign-editor/index.html";
  } else if (decoded.startsWith("/tools/campaign-editor/")) {
    relative = "sow-tools/editors/campaign-editor/" + decoded.slice("/tools/campaign-editor/".length);
  }
  if (!relative.startsWith("assets/") && !relative.startsWith("sow-tools/editors/campaign-editor/")) return null;
  const file = path.resolve(root, relative);
  return file === root || file.startsWith(root + path.sep) ? file : null;
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

async function validateCampaign(file, value) {
  const match = file.match(/^([a-z0-9]+(?:_[a-z0-9]+)*)\.json$/);
  const triggerMatch = file.match(/^([a-z0-9]+(?:_[a-z0-9]+)*)\.triggers\.json$/);
  if (match) {
    if (!value || typeof value.map !== "string" || !Array.isArray(value.player_spawn) || value.player_spawn.length !== 2 || !Array.isArray(value.factions) || !value.factions.length) throw new Error("invalid roster");
    if (!value.player_spawn.every(Number.isFinite)) throw new Error("invalid player spawn");
    const names = new Set();
    for (const faction of value.factions || []) {
      if (!faction || typeof faction.name !== "string" || !faction.name.trim() || names.has(faction.name)) throw new Error("duplicate faction");
      if (!roles.has(faction.role) || !Number.isFinite(faction.x) || !Number.isFinite(faction.y)) throw new Error("invalid faction");
      names.add(faction.name);
    }
    return;
  }
  if (triggerMatch) {
    const episodeId = triggerMatch[1];
    if (!value || value.version !== 1 || value.episode_id !== episodeId || !Array.isArray(value.steps) || !value.steps.length) throw new Error("invalid campaign steps");
    if (!value.settings || typeof value.settings.buildings_enabled !== "boolean" || !Number.isFinite(Number(value.settings.starting_troops)) || Number(value.settings.starting_troops) < 1 || Number(value.settings.starting_troops) > 100000) throw new Error("invalid campaign settings");
    const ids = new Set();
    for (const step of value.steps) {
      if (!step || typeof step.id !== "string" || !step.id || ids.has(step.id)) throw new Error("duplicate step id");
      if (!step.trigger || !triggerTypes.has(step.trigger.type)) throw new Error("unknown trigger type");
      if (["territory", "kills", "attack", "troops", "building", "fleet", "nuke", "elapsed"].includes(step.trigger.type) && !Number.isFinite(Number(step.trigger.value))) throw new Error("invalid trigger value");
      if (![step.title_key, step.body_key, step.hint_key].every(key => typeof key === "string" && key.startsWith("tutorial."))) throw new Error("invalid translation key");
      if (step.marker && typeof step.marker.target !== "string") throw new Error("invalid marker");
      for (const action of [...(step.on_enter || []), ...(step.on_complete || [])]) {
        if (!action || !actionTypes.has(action.type)) throw new Error("unknown action type");
      }
      ids.add(step.id);
    }
    const rosterPath = path.join(campaignDir, `${episodeId}.json`);
    try {
      const roster = JSON.parse(await fs.readFile(rosterPath, "utf8"));
      const names = new Set((roster.factions || []).map(faction => faction.name));
      for (const step of value.steps) {
        const targets = [step.trigger.target, step.marker && step.marker.target].filter(target => target && target !== "player");
        if (targets.some(target => !names.has(target))) throw new Error("step references an unknown faction");
      }
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    return;
  }
  throw new Error("bad file name");
}

async function handle(req, res) {
  const url = new URL(req.url, "http://127.0.0.1");
  if (req.method === "POST" && url.pathname === "/__save") {
    const file = url.searchParams.get("file") || "";
    const body = await readBody(req);
    try {
      await validateCampaign(file, JSON.parse(body.toString("utf8")));
    } catch (error) {
      reply(res, 400, error.message || "invalid JSON");
      return;
    }
    await fs.mkdir(campaignDir, { recursive: true });
    await fs.writeFile(path.join(campaignDir, file), body);
    console.log(`  saved assets/campaign/${file}`);
    reply(res, 200, `saved ${file}`);
    return;
  }
  if (req.method !== "GET" && req.method !== "HEAD") {
    reply(res, 405, "method not allowed");
    return;
  }
  let file;
  try {
    file = staticFile(url.pathname);
  } catch {
    reply(res, 400, "bad path");
    return;
  }
  if (!file) {
    reply(res, 404, "not found");
    return;
  }
  try {
    const body = await fs.readFile(file);
    res.writeHead(200, { "Content-Type": contentType(file), "Cache-Control": "no-store" });
    res.end(req.method === "HEAD" ? undefined : body);
  } catch {
    reply(res, 404, "not found");
  }
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
  console.log("  Export writes assets/campaign/ directly; refresh the web preview to load changes.");
  console.log("  Ctrl-C to stop.");
});
