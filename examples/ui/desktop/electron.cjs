const { app, BrowserWindow, protocol, session } = require('electron');
const path = require('node:path');
const { realpath, readFile } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const { randomUUID } = require('node:crypto');

protocol.registerSchemesAsPrivileged([{ scheme: 'ubi', privileges: { standard: true, secure: true, supportFetchAPI: true } }]);
const smoke = process.argv.includes('--smoke-test');
if (smoke) app.setPath('userData', path.join(tmpdir(), `ubi-ui-smoke-${randomUUID()}`));
let window;
app.whenReady().then(async () => {
  const root = await realpath(__dirname);
  protocol.handle('ubi', async request => {
    try {
      const url = new URL(request.url);
      if (url.hostname !== 'app' || url.username || url.password || request.method !== 'GET') return new Response('Forbidden', { status: 403 });
      const name = decodeURIComponent(url.pathname.slice(1)) || 'index.html';
      if (name.includes('\\') || name.split('/').some(part => part === '..' || part === '.')) return new Response('Forbidden', { status: 403 });
      const file = await realpath(path.join(root, name));
      const relative = path.relative(root, file);
      if (relative.startsWith('..') || path.isAbsolute(relative)) return new Response('Forbidden', { status: 403 });
      if (!['index.html', 'app.js', 'storage.js', 'style.css', 'renderer.mjs', 'codec.mjs', 'validate.mjs'].includes(name) && !name.startsWith('modules/')) return new Response('Forbidden', { status: 403 });
      const contentType = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css' }[path.extname(file)] ?? 'application/octet-stream';
      return new Response(await readFile(file), { headers: { 'Content-Type': contentType } });
    } catch { return new Response('Not found', { status: 404 }); }
  });
  session.defaultSession.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  session.defaultSession.setPermissionCheckHandler(() => false);
  window = new BrowserWindow({ width: 850, height: 900, minWidth: 320, minHeight: 500, show: !smoke,
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false, webSecurity: true },
  });
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.on('will-navigate', event => event.preventDefault());
  window.webContents.on('will-redirect', event => event.preventDefault());
  window.webContents.on('will-attach-webview', event => event.preventDefault());
  window.webContents.session.webRequest.onBeforeRequest((details, callback) => callback({ cancel: !details.url.startsWith('ubi://app/') }));
  await window.loadURL('ubi://app/index.html');
  if (smoke) {
    const passed = await window.webContents.executeJavaScript(`(async () => {
      const start = Date.now();
      const get = key => document.querySelector('[data-ubi-key="' + key + '"]');
      while (!get('primary') && Date.now() - start < 10000) await new Promise(resolve => setTimeout(resolve, 50));
      if (!get('primary') || typeof window.require !== 'undefined' || typeof window.process !== 'undefined') return false;
      get('primary').click();
      if (get('interaction-count')?.textContent !== '1 interactions') return false;
      get('theme-dark').click();
      if (get('explorer').dataset.ubiTheme !== 'dark') return false;
      get('nav-composition').click();
      if (get('project-count')?.textContent !== '1') return false;
      get('reset-ideas').click();
      if (get('project-count')?.textContent !== '0' || !document.querySelector('#error-boundary').hidden) return false;
      const forbidden = await fetch('ubi://app/package.json');
      console.log('UBI_UI_ENGINE ' + navigator.userAgent);
      return forbidden.status === 403;
    })()`);
    console.log(passed ? 'UBI_UI_DESKTOP_SMOKE_OK' : 'UBI_UI_DESKTOP_SMOKE_FAILED');
    app.exit(passed ? 0 : 1);
  }
}).catch(cause => { console.error(cause); app.exit(1); });
app.on('window-all-closed', () => app.quit());
