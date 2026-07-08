// Static file server for the built app. Port 0 = ephemeral (measure/coverage runs);
// `/blank.html` is synthesized so coverage can arm the profiler on a same-origin page
// before index.html loads (keeps the renderer process, and thus V8 coverage, alive).

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
};

const BLANK_HTML = '<!doctype html><title>blank</title>ok';

export function startServer(rootDir, port = 0) {
  const server = http.createServer((req, res) => {
    let pathname;
    try {
      pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    } catch {
      res.writeHead(400).end('bad request');
      return;
    }
    if (pathname === '/blank.html') {
      res.writeHead(200, { 'content-type': MIME['.html'], 'cache-control': 'no-store' });
      res.end(BLANK_HTML);
      return;
    }
    if (pathname === '/') pathname = '/index.html';
    const file = path.normalize(path.join(rootDir, pathname));
    if (!file.startsWith(path.normalize(rootDir)) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      res.writeHead(404).end('not found');
      return;
    }
    res.writeHead(200, {
      'content-type': MIME[path.extname(file).toLowerCase()] ?? 'application/octet-stream',
      'cache-control': 'no-store',
    });
    fs.createReadStream(file).pipe(res);
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
