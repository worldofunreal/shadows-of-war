// Sow Spritesheet Viewer — entry point de Electron.
//
// Ventana para visor de spritesheets animadas:
//   - Abrir spritesheet: botón del header, Ctrl+O (menú) o drag & drop sobre
//     la hoja principal.
//   - Detecta la cuadrícula automáticamente (grid.js) y se puede editar a
//     mano (Aplicar / Auto / Swap).
//   - Reproduce los marcos marco a marco a FPS (slider o campo numérico).
//   - Reproducir/Pausar, paso manual (◀ ▶) y saltos con el clic sobre la
//     vista de la hoja entera.
//
// Menú de desarrollador: Pantalla completa (F11), Herramientas de
// desarrollador (F12), Recargar (Ctrl+R / F5), Recargar sin caché
// (Ctrl+Shift+R), Inspect.

const { app, BrowserWindow, Menu, dialog } = require('electron');
const path = require('path');
const fs = require('fs');

if (process.platform === 'linux') {
  app.commandLine.appendSwitch('disable-gpu-sandbox');
}

// '/home/u/hoja.png' → 'file:///home/u/hoja.png'
function fileUrlFromPath(p) {
  return 'file://' + p.split(path.sep).map(encodeURIComponent).join('/');
}

function openSpritesheet(win) {
  dialog.showOpenDialog(win, {
    title: 'Abrir spritesheet',
    properties: ['openFile'],
    filters: [
      { name: 'Spritesheets e imágenes', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'avif', 'bmp', 'heic'] },
    ],
  }).then((result) => {
    const [p] = result.filePaths || [];
    if (!p) return;
    if (!fs.existsSync(p)) {
      console.error('[sow-sprites] archivo inexistente:', p);
      return;
    }
    win
      .webContents.executeJavaScript(
        `window.loadSpritesheetFromMain(${JSON.stringify(fileUrlFromPath(p))}, ${JSON.stringify(path.basename(p))})`
      )
      .catch((err) => console.error('[sow-sprites] no se pudo cargar:', err.message));
  });
}

function toggleDevTools(win) {
  win.webContents.toggleDevTools();
}

function reload(win) {
  win.webContents.reload();
}

function reloadNoCache(win) {
  win.webContents.reloadIgnoringCache();
}

function setFullscreen(win) {
  win.setFullScreen(!win.isFullScreen());
}

function buildMenu(win) {
  return Menu.buildFromTemplate([
    {
      label: 'Archivo',
      submenu: [
        { label: 'Abrir spritesheet…', accelerator: 'CmdOrCtrl+O', click: () => openSpritesheet(win) },
        { type: 'separator' },
        { role: 'quit', label: 'Salir' },
      ],
    },
    {
      label: 'Ver',
      submenu: [
        { label: 'Pantalla completa', accelerator: 'F11', click: () => setFullscreen(win) },
        { type: 'separator' },
        { label: 'Herramientas de desarrollador', accelerator: 'F12', click: () => toggleDevTools(win) },
        { label: 'Recargar', accelerator: 'CmdOrCtrl+R', click: () => reload(win) },
        { label: 'Recargar sin caché', accelerator: 'CmdOrCtrl+Shift+R', click: () => reloadNoCache(win) },
      ],
    },
    {
      label: 'Ayuda',
      submenu: [
        {
          label: 'Acerca de…',
          click: () => {
            const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, 'package.json'), 'utf8'));
            dialog.showMessageBox(win, {
              type: 'info',
              title: 'Sow Spritesheet Viewer',
              message: `${pkg.name} v${pkg.version}\nVisor de spritesheet animado`,
              buttons: ['OK'],
            });
          },
        },
      ],
    },
  ]);
}

let win = null;

app.whenReady().then(() => {
  win = new BrowserWindow({
    width: 1120,
    height: 780,
    minWidth: 640,
    minHeight: 480,
    title: 'Sow Spritesheet Viewer',
    backgroundColor: '#0e0e12',
    autoHideMenuBar: true,
    webPreferences: { contextIsolation: true, nodeIntegration: false },
  });
  win.setMenu(buildMenu(win));
  win.loadFile(path.join(__dirname, 'index.html'));

  win.webContents.on('console-message', (event) => {
    const message = event && (event.message ?? event[2]);
    if (message) console.log(`[console] ${message} (${event.sourceId}:${event.lineNumber})`);
  });
  win.webContents.on('did-fail-load', (_event, code, desc, validatedURL) => {
    console.error(`[did-fail-load] ${code} ${desc} (${validatedURL})`);
  });
});

app.on('window-all-closed', () => {
  app.quit();
});
