const { app, BrowserWindow, protocol, net } = require('electron');
const path = require('path');
const url = require('url');
const fs = require('fs');

const webroot = process.argv[2] || process.env.SOW_WEBROOT || path.join(__dirname, 'web');

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

app.whenReady().then(() => {
  protocol.handle('sow', (request) => {
    try {
      const parsed = new URL(request.url);
      const decodedPath = decodeURIComponent(parsed.pathname).replace(/^\/+/, '');
      const fullPath = path.join(webroot, decodedPath);

      if (!fs.existsSync(fullPath)) {
        return new Response('File not found', { status: 404 });
      }

      return net.fetch(url.pathToFileURL(fullPath).toString());
    } catch {
      return new Response('Internal error', { status: 500 });
    }
  });

  const iconPath = path.join(__dirname, 'icons', '128x128.png');

  const win = new BrowserWindow({
    width: 1280,
    height: 720,
    backgroundColor: '#0a0a0f',
    autoHideMenuBar: true,
    title: 'Shadows of War',
    icon: fs.existsSync(iconPath) ? iconPath : undefined,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      webSecurity: true,
    },
  });

  win.webContents.on('before-input-event', (event, input) => {
    if (input.key === 'F11' && input.type === 'keyDown') {
      win.setFullScreen(!win.isFullScreen());
      event.preventDefault();
    }
  });

  win.loadURL('sow://app/play/index.html');
});

app.on('window-all-closed', () => {
  app.quit();
});
