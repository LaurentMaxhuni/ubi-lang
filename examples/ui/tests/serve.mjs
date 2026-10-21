// Local browser harness only; binds loopback and serves the tests/web directories.
import http from 'node:http';
import { readFile, realpath } from 'node:fs/promises';
import { dirname, extname, resolve, relative, isAbsolute } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = await realpath(resolve(dirname(fileURLToPath(import.meta.url)), '..'));
http.createServer(async (request, response) => {
  try {
    if (request.method !== 'GET') throw Error('GET only');
    const name = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    if (!/^\/(tests|web)\//.test(name) || name.includes('\\') || name.split('/').some(part => part === '..' || part === '.')) throw Error('Invalid path');
    const file = await realpath(resolve(root, `.${name}`));
    const within = relative(root, file);
    if (within.startsWith('..') || isAbsolute(within)) throw Error('Outside root');
    response.setHeader('Content-Type', ({ '.mjs': 'text/javascript', '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html' })[extname(file)] ?? 'text/plain');
    response.end(await readFile(file));
  } catch { response.writeHead(404); response.end('Not found'); }
}).listen(8791, '127.0.0.1', () => console.log('http://127.0.0.1:8791/tests/browser.html'));
