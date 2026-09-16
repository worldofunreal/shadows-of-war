import http from "node:http";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

const editorDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(editorDir, "../../..");
const campaignDir = path.join(root, "assets/campaign");
const allowedSaves = new Set(["boudica.json", "boudica.triggers.json"]);
const maxBodyBytes = 1024 * 1024;

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

async function launchClient(res) {
  const targetDir = path.resolve(root, process.env.CARGO_TARGET_DIR || "target");
  const client = path.join(targetDir, "debug", process.platform === "win32" ? "client.exe" : "client");
  try {
    await fs.access(client);
  } catch {
    reply(res, 409, `Client not found: ${client}. Run ./sow native once.`);
    return;
  }
  const child = spawn(client, [], {
    cwd: root,
    detached: true,
    stdio: "inherit",
    env: { ...process.env, VERBOSE: process.env.VERBOSE || "1" }
  });
  child.once("error", error => console.error(`client launch failed: ${error.message}`));
  child.unref();
  reply(res, 200, "launched");
}

async function handle(req, res) {
  const url = new URL(req.url, "http://127.0.0.1");
  if (req.method === "POST" && url.pathname === "/__save") {
    const file = url.searchParams.get("file") || "";
    if (!allowedSaves.has(file)) {
      reply(res, 400, "bad file name");
      return;
    }
    const body = await readBody(req);
    try {
      JSON.parse(body.toString("utf8"));
    } catch {
      reply(res, 400, "invalid JSON");
      return;
    }
    await fs.mkdir(campaignDir, { recursive: true });
    await fs.writeFile(path.join(campaignDir, file), body);
    console.log(`  saved assets/campaign/${file}`);
    reply(res, 200, `saved ${file}`);
    return;
  }
  if (req.method === "POST" && url.pathname === "/__launch") {
    await launchClient(res);
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
  console.log("  Export writes assets/campaign/ directly; launch uses the existing client binary.");
  console.log("  Ctrl-C to stop.");
});
