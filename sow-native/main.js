const { app, BrowserWindow, protocol, net } = require('electron');
const path = require('path');
const url = require('url');
const fs = require('fs');

if (process.platform === 'linux') {
  app.commandLine.appendSwitch('disable-gpu-sandbox');
}

const webroot = process.argv[2] || process.env.SOW_WEBROOT || path.join(__dirname, 'web');
const liveRoot = process.env.SOW_LIVE_ROOT || '';
const liveAssets = process.env.SOW_LIVE_ASSETS || '';

const VIEWPORT_PRESETS = [
  { name: 'Desktop', width: 1280, height: 720 },
  { name: 'Mobile Portrait', width: 390, height: 844 },
  { name: 'Mobile Landscape', width: 844, height: 390 },
];

protocol.registerSchemesAsPrivileged([
  {
    scheme: 'sow',
    privileges: {
      standard: true,
      secure: true,
      supportFetchAPI: true,
      corsEnabled: true,
      stream: true,
    },
  },
]);

function resolveSowPath(decodedPath) {
  if (liveRoot) {
    if (decodedPath.startsWith('assets/campaign/')) {
      return path.join(liveRoot, decodedPath);
    }
    if (decodedPath.startsWith('assets/guides/')) {
      return path.join(liveRoot, decodedPath);
    }
    if (decodedPath.startsWith('maps/')) {
      return path.join(liveRoot, 'assets', decodedPath);
    }
    if (decodedPath.startsWith('fonts/')) {
      return path.join(liveRoot, 'sow-web/site', decodedPath);
    }
    if (
      liveAssets &&
      (decodedPath.startsWith('assets/gameplay/') || decodedPath.startsWith('assets/shell/'))
    ) {
      const candidate = path.join(liveAssets, decodedPath.slice('assets/'.length));
      if (fs.existsSync(candidate)) return candidate;
    }
  }
  return path.join(webroot, decodedPath);
}

app.whenReady().then(() => {
  protocol.handle('sow', async (request) => {
    try {
      const parsed = new URL(request.url);
      const decodedPath = decodeURIComponent(parsed.pathname).replace(/^\/+/, '');
      const fullPath = resolveSowPath(decodedPath);

      if (!fs.existsSync(fullPath)) {
        if (liveRoot) {
          console.error(`[sow:// 404] /${decodedPath} -> ${fullPath}`);
        }
        return new Response('File not found', { status: 404 });
      }

      const response = await net.fetch(url.pathToFileURL(fullPath).toString());
      if (decodedPath.endsWith('.wasm')) {
        const headers = new Headers(response.headers);
        headers.set('Content-Type', 'application/wasm');
        return new Response(response.body, { status: response.status, headers });
      }
      return response;
    } catch (err) {
      if (liveRoot) {
        console.error(`[sow:// 500] ${request.url}:`, err);
      }
      return new Response('Internal error', { status: 500 });
    }
  });

  const iconPath = path.join(__dirname, 'icons', '128x128.png');
  let presetIndex = process.argv.includes('--mobile') ? 1 : 0;
  const initial = VIEWPORT_PRESETS[presetIndex];
  const verifyBoot = process.env.SOW_DEV_VERIFY_BOOT === '1';

  const win = new BrowserWindow({
    width: initial.width,
    height: initial.height,
    show: !verifyBoot,
    backgroundColor: '#0a0a0f',
    autoHideMenuBar: true,
    title: liveRoot ? `Shadows of War [DEV · ${initial.name}]` : 'Shadows of War',
    icon: fs.existsSync(iconPath) ? iconPath : undefined,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      webSecurity: true,
    },
  });

  if (liveRoot) {
    win.webContents.on('console-message', (event, legacyLevel, legacyMsg, legacyLine, legacySrc) => {
      const isDetails = event && typeof event.message === 'string';
      const level = isDetails ? event.level : legacyLevel;
      const message = isDetails ? event.message : legacyMsg;
      const sourceId = isDetails ? event.sourceId : legacySrc;
      const lineNumber = isDetails ? event.lineNumber : legacyLine;
      const isErrOrWarn =
        level === 'error' || level === 'warning' || (typeof level === 'number' && level >= 2);
      if (isErrOrWarn || message.startsWith('[SOW')) {
        const prefix = `[console:${level}]`;
        const loc = sourceId ? ` (${sourceId}:${lineNumber})` : '';
        (isErrOrWarn ? console.error : console.log)(`${prefix} ${message}${loc}`);
      }
    });
    win.webContents.on('did-fail-load', (_e, code, desc, validatedURL) => {
      console.error(`[Electron did-fail-load] ${code} ${desc} (${validatedURL})`);
    });
    win.webContents.on('render-process-gone', (_e, details) => {
      console.error(`[Electron render-process-gone]`, details);
    });
    const stampPath = path.join(webroot, '.reload-stamp');
    let reloadTimer = null;
    if (fs.existsSync(stampPath)) {
      fs.watch(stampPath, () => {
        if (reloadTimer) clearTimeout(reloadTimer);
        reloadTimer = setTimeout(() => {
          reloadTimer = null;
          if (!win.isDestroyed()) win.webContents.reloadIgnoringCache();
        }, 80);
      });
    }
  }

  win.webContents.on('before-input-event', (event, input) => {
    if (input.type !== 'keyDown') return;
    if (input.key === 'F11') {
      win.setFullScreen(!win.isFullScreen());
      event.preventDefault();
    } else if (input.key === 'F5' || (input.control && !input.shift && input.key.toLowerCase() === 'r')) {
      win.webContents.reloadIgnoringCache();
      event.preventDefault();
    } else if (input.control && input.shift && input.key.toLowerCase() === 'r') {
      win.webContents.session
        .clearStorageData({ storages: ['localstorage', 'caches'] })
        .then(() => {
          if (!win.isDestroyed()) win.webContents.reloadIgnoringCache();
        });
      event.preventDefault();
    } else if (input.key === 'F12') {
      win.webContents.toggleDevTools();
      event.preventDefault();
    } else if (input.key === 'F9') {
      presetIndex = (presetIndex + 1) % VIEWPORT_PRESETS.length;
      const preset = VIEWPORT_PRESETS[presetIndex];
      if (win.isFullScreen()) win.setFullScreen(false);
      win.setSize(preset.width, preset.height);
      win.center();
      if (liveRoot) win.setTitle(`Shadows of War [DEV · ${preset.name}]`);
      event.preventDefault();
    }
  });

  const loadTarget = verifyBoot
    ? 'sow://app/play/index.html?diag=1'
    : 'sow://app/play/index.html';
  win.loadURL(loadTarget);

  if (verifyBoot) {
    const start = Date.now();
    const poll = setInterval(async () => {
      if (win.isDestroyed()) {
        clearInterval(poll);
        return;
      }
      try {
        const state = await win.webContents.executeJavaScript(`({
          menuReady: !!window.SOW_MENU_READY,
          loaderHidden: !document.getElementById('web-loader') || getComputedStyle(document.getElementById('web-loader')).display === 'none' || document.getElementById('web-loader').hidden,
          hasWebgl2: typeof window.SOW_has_webgl2 === 'function' ? window.SOW_has_webgl2() : null,
          mapsUrl: window.SOW_MAPS_URL || null,
          assetsUrl: window.SOW_ASSETS_URL || null,
        })`);
        if (state.menuReady && state.loaderHidden) {
          console.log(`[verify-boot] OK in ${Date.now() - start}ms`, JSON.stringify(state));
          clearInterval(poll);
          app.exit(0);
        } else if (Date.now() - start > 15000) {
          console.error(`[verify-boot] TIMEOUT after ${Date.now() - start}ms`, JSON.stringify(state));
          clearInterval(poll);
          app.exit(1);
        }
      } catch (err) {
        if (Date.now() - start > 15000) {
          console.error('[verify-boot] ERROR:', err);
          clearInterval(poll);
          app.exit(1);
        }
      }
    }, 250);
  }
});

app.on('window-all-closed', () => {
  app.quit();
});
