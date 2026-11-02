// Static file server for the built app. Port 0 = ephemeral (measure/coverage runs);
// `/blank.html` is synthesized so coverage can arm the profiler on a same-origin page
// before index.html loads (keeps the renderer process, and thus V8 coverage, alive).

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
};

// Cross-origin isolation: wasm-threaded apps (SQLite wasm, ffmpeg.wasm) need
// SharedArrayBuffer, which browsers only enable under COOP+COEP — without these
// headers such apps show a fatal error instead of a first paint (found via
// Actual Budget). COEP 'credentialless' (not 'require-corp') keeps cross-origin
// subresources like third-party fonts loading, so non-wasm apps measure
// identically to before.
const ISOLATION_HEADERS = {
  'cross-origin-opener-policy': 'same-origin',
  'cross-origin-embedder-policy': 'credentialless',
  'cross-origin-resource-policy': 'cross-origin',
};

// Real deployments compress text assets; serving a 16MB bundle raw would
// overcharge the throttled lab by 3-4x versus what any user's wire sees
// (found via drawDB). transfer_bytes reads encodedBodySize, so metrics report
// compressed bytes - same as production.
const COMPRESSIBLE = new Set([
  '.html',
  '.js',
  '.mjs',
  '.css',
  '.json',
  '.map',
  '.svg',
  '.txt',
  '.webmanifest',
  '.wasm',
]);

function wantsGzip(req, ext) {
  return COMPRESSIBLE.has(ext) && /\bgzip\b/.test(req.headers['accept-encoding'] ?? '');
}

// Deliberately contentless: the warm-up page must never fire FCP/LCP, or a
// coverage/measure poll racing the next navigation could read the blank page's
// paint entries as the app's.
const BLANK_HTML = '<!doctype html><title>blank</title>';

// Navigations get what a real static host gives them: a directory's index.html, a
// sibling .html for an extensionless path (multi-page entries), and finally the
// root index.html (SPA history routes, which parity loads by path). Subresource
// misses stay 404 - an API fetch must never start receiving HTML it didn't get
// before, or a measurement would change under the app.
function resolveFile(rootDir, pathname, navigation) {
  const inRoot = (file) =>
    (file === rootDir || file.startsWith(rootDir + path.sep)) &&
    fs.existsSync(file) &&
    fs.statSync(file).isFile();
  const direct = path.normalize(path.join(rootDir, pathname === '/' ? '/index.html' : pathname));
  if (inRoot(direct)) return direct;
  if (!navigation) return null;
  const fallbacks = [path.join(direct, 'index.html')];
  if (!path.extname(pathname)) fallbacks.push(`${direct}.html`, path.join(rootDir, 'index.html'));
  return fallbacks.map((file) => path.normalize(file)).find(inRoot) ?? null;
}

/**
 * `root` may be a function: parity serves the original build and the candidate
 * from ONE origin (neither may render a different `location`), swapping the root
 * between captures.
 */
export function startServer(root, port = 0) {
  const rootOf = typeof root === 'function' ? root : () => root;
  const server = http.createServer((req, res) => {
    let pathname;
    try {
      pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    } catch {
      res.writeHead(400).end('bad request');
      return;
    }
    if (pathname === '/blank.html') {
      res.writeHead(200, {
        'content-type': MIME['.html'],
        'cache-control': 'no-store',
        ...ISOLATION_HEADERS,
      });
      res.end(BLANK_HTML);
      return;
    }
    const file = resolveFile(
      path.normalize(rootOf()),
      pathname,
      req.headers['sec-fetch-mode'] === 'navigate',
    );
    if (!file) {
      res.writeHead(404).end('not found');
      return;
    }
    const ext = path.extname(file).toLowerCase();
    const headers = {
      'content-type': MIME[ext] ?? 'application/octet-stream',
      'cache-control': 'no-store',
      ...ISOLATION_HEADERS,
    };
    if (wantsGzip(req, ext)) {
      res.writeHead(200, { ...headers, 'content-encoding': 'gzip' });
      fs.createReadStream(file)
        .pipe(zlib.createGzip({ level: 6 }))
        .pipe(res);
    } else {
      res.writeHead(200, headers);
      fs.createReadStream(file).pipe(res);
    }
  });
  return new Promise((resolve, reject) => {
    server.on('error', reject);
    server.listen(port, '127.0.0.1', () => {
      const actualPort = server.address().port;
      resolve({
        port: actualPort,
        origin: `http://127.0.0.1:${actualPort}`,
        close: () => new Promise((r) => server.close(r)),
      });
    });
  });
}
