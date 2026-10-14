// The replay server: one HTTPS + HTTP/2 server for EVERY host the page uses.
// Chrome reaches it through --host-resolver-rules, so the page runs at its real
// origin (cookies, CORS, absolute URLs all unchanged) and no request leaves the
// machine. What it serves, in this order:
//   document   a navigation to the recorded origin: the local dist's index.html
//              plus the tags only the recorded HTML has (mergeDocument), or the
//              bare local / recorded HTML, after the recorded document TTFB
//   preflight  a CORS preflight: the recorded one, or an allow-all answer after one RTT
//   dist       a file of the local dist on the recorded origin, after an idle-link
//              time to first byte drawn from the recorded ones, then behind the
//              bytes already on the wire; compressed like the recorded assets
//   recorded   a recorded response (see createResponseIndex) after its recorded
//              time to first byte
//   miss       anything else: 404 after one RTT, and a line in the run's log
// Every body goes through the shared bandwidth shaper.

import fs from 'node:fs';
import http2 from 'node:http2';
import path from 'node:path';
import zlib from 'node:zlib';

import { createResponseIndex, mergeDocument, sampleQuantiles, seededRandom } from './model.mjs';
import { createShaper, urgencyOf } from './shaper.mjs';

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.webmanifest': 'application/manifest+json',
  '.map': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.webp': 'image/webp',
  '.avif': 'image/avif',
  '.ico': 'image/x-icon',
  '.gif': 'image/gif',
  '.woff2': 'font/woff2',
  '.woff': 'font/woff',
  '.ttf': 'font/ttf',
  '.wasm': 'application/wasm',
  '.txt': 'text/plain; charset=utf-8',
};
const COMPRESSIBLE = new Set([
  '.html',
  '.js',
  '.mjs',
  '.css',
  '.json',
  '.webmanifest',
  '.map',
  '.svg',
  '.txt',
  '.wasm',
  '.ico',
  '.ttf',
]);
const DEFAULT_LEVEL = { br: 5, gzip: 6, zstd: 3 };
const NO_BODY = new Set([204, 205, 304]);

export function encode(data, { encoding, level }) {
  const lvl = level ?? DEFAULT_LEVEL[encoding];
  if (encoding === 'br') {
    return zlib.brotliCompressSync(data, {
      params: {
        [zlib.constants.BROTLI_PARAM_QUALITY]: lvl,
        [zlib.constants.BROTLI_PARAM_SIZE_HINT]: data.length,
      },
    });
  }
  if (encoding === 'gzip') return zlib.gzipSync(data, { level: lvl });
  if (encoding === 'zstd')
    return zlib.zstdCompressSync(data, {
      params: { [zlib.constants.ZSTD_c_compressionLevel]: lvl },
    });
  return data;
}

const LEVELS = { br: [1, 3, 4, 5, 6, 9, 11], gzip: [1, 4, 6, 9], zstd: [1, 3, 6, 9, 12, 19] };

/** The file of `dist` that serves `pathname`, or null (outside `dist`, missing, a folder). */
export function distFile(dist, pathname) {
  let decoded;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    return null;
  }
  const file = path.normalize(path.join(dist, decoded));
  if (!file.startsWith(path.normalize(dist) + path.sep)) return null;
  try {
    return fs.statSync(file).isFile() ? file : null;
  } catch {
    return null;
  }
}

/**
 * The encoding and level that reproduce the recorded asset sizes best, from up
 * to `sample` of the largest recorded assets that also exist in `dist` (the
 * recorded build, ideally). Header bytes in the recorded sizes are ignored, so
 * only assets of 20 kB or more count.
 */
export function chooseCompression(assets, dist, { sample = 8 } = {}) {
  const rows = assets
    .filter(
      (a) =>
        a.encoding &&
        a.encodedBytes >= 20_000 &&
        LEVELS[a.encoding] &&
        COMPRESSIBLE.has(path.extname(a.path)),
    )
    .map((a) => ({ ...a, file: distFile(dist, a.path) }))
    .filter((a) => a.file)
    .sort((a, b) => b.encodedBytes - a.encodedBytes)
    .slice(0, sample);
  if (!rows.length)
    return {
      encoding: null,
      level: null,
      files: 0,
      note: 'no recorded compressed asset exists in the dist',
    };
  const counts = new Map();
  for (const r of rows) counts.set(r.encoding, (counts.get(r.encoding) ?? 0) + 1);
  const [encoding] = [...counts].sort((a, b) => b[1] - a[1])[0];
  const mine = rows.filter((r) => r.encoding === encoding);
  const data = mine.map((r) => fs.readFileSync(r.file));
  let best = null;
  for (const level of LEVELS[encoding]) {
    const ratios = mine
      .map((r, i) => r.encodedBytes / encode(data[i], { encoding, level }).length)
      .sort((a, b) => a - b);
    const mid = ratios[Math.floor(ratios.length / 2)];
    if (!best || Math.abs(Math.log(mid)) < Math.abs(Math.log(best.ratio)))
      best = { level, ratio: mid };
  }
  return {
    encoding,
    level: best.level,
    files: mine.length,
    recordedOverLocal: Math.round(best.ratio * 1000) / 1000,
  };
}

/**
 * HTML split like the recorded document arrived: the early share right after
 * the headers (cut at the end of a tag), the rest when the recorded download
 * ended. Sent uncompressed so that both parts decode at once; on the order of
 * 10 kB of HTML, compression changes nothing measurable.
 */
export function documentSegments(html, doc) {
  const ttfb = doc.ttfbMs ?? 0;
  const f = doc.earlyFraction;
  if (typeof f !== 'number' || f >= 1 || !doc.downloadMs) return [{ atMs: ttfb, data: html }];
  let split = 0;
  if (f > 0) {
    const gt = html.indexOf(0x3e, Math.floor(html.length * f));
    split = gt === -1 ? html.length : gt + 1;
  }
  return [
    { atMs: ttfb, data: html.subarray(0, split) },
    { atMs: ttfb + doc.downloadMs, data: html.subarray(split) },
  ];
}

const readBody = (req) =>
  new Promise((resolve) => {
    const chunks = [];
    req.on('data', (c) => chunks.push(c));
    req.on('end', () => resolve(chunks.length ? Buffer.concat(chunks).toString('utf8') : null));
    req.on('error', () => resolve(null));
  });

const accepts = (req, encoding) =>
  encoding != null &&
  new RegExp(`(^|[,\\s])${encoding}(;|,|\\s|$)`).test(req.headers['accept-encoding'] ?? '');

export async function startReplayServer({
  archive,
  archiveDir,
  tls,
  network,
  compression = null,
  documentSource = 'merged',
}) {
  const index = createResponseIndex(archive.endpoints, archive.volatile);
  const shaper = createShaper({
    bytesPerSec: network.bandwidthBps,
    wireBytes: network.wireBytes ?? 0,
  });
  const rttMs = network.rttMs ?? 0;
  const bodyCache = new Map();
  const encodedCache = new Map();
  let dist = null;
  let log = [];
  let rand = seededRandom(0);

  const recordedBody = (ref) => {
    if (!ref) return Buffer.alloc(0);
    if (!bodyCache.has(ref))
      bodyCache.set(ref, fs.readFileSync(path.join(archiveDir, 'bodies', ref)));
    return bodyCache.get(ref);
  };
  const encodedOnce = (cacheKey, data, how) => {
    if (!encodedCache.has(cacheKey)) encodedCache.set(cacheKey, encode(data, how));
    return encodedCache.get(cacheKey);
  };

  /**
   * Headers after `delayMs`, then the body through the shaper. `segments`
   * ([{ atMs, data }], times from the request's arrival) sends the body in
   * parts, like a host that flushes the head of the HTML early.
   */
  async function respond(
    req,
    res,
    entry,
    {
      status,
      headers = {},
      body,
      segments = null,
      delayMs,
      holdMs = 0,
      queueHeaders = false,
      encodeAs = null,
      cacheKey = null,
    },
  ) {
    const wait = entry.t + delayMs - performance.now();
    if (wait > 0) await new Promise((r) => setTimeout(r, wait));
    // Static headers queue behind the bytes already on the wire (see shaper.mjs).
    if (queueHeaders) await shaper.header();
    if (res.destroyed || res.closed) {
      entry.cancelled = true;
      return;
    }
    const out = { ...headers };
    delete out['content-length'];
    delete out['content-encoding'];
    let parts = segments ?? [{ atMs: 0, data: body ?? Buffer.alloc(0) }];
    if (
      !segments &&
      encodeAs?.encoding &&
      parts[0].data.length &&
      accepts(req, encodeAs.encoding)
    ) {
      const data = parts[0].data;
      parts = [
        {
          atMs: 0,
          data: cacheKey
            ? encodedOnce(`${cacheKey}|${encodeAs.encoding}|${encodeAs.level}`, data, encodeAs)
            : encode(data, encodeAs),
        },
      ];
      out['content-encoding'] = encodeAs.encoding;
    }
    // A late body: everything but the last byte now, the last byte `holdMs`
    // after the headers. A script or JSON response is usable only when complete.
    if (!segments && holdMs > 0 && parts[0].data.length > 1) {
      const data = parts[0].data;
      parts = [
        { atMs: 0, data: data.subarray(0, data.length - 1) },
        { atMs: delayMs + holdMs, data: data.subarray(data.length - 1) },
      ];
    }
    const total = parts.reduce((s, p) => s + p.data.length, 0);
    const noBody = req.method === 'HEAD' || NO_BODY.has(status);
    if (!noBody) out['content-length'] = String(total);
    out['cache-control'] = 'no-store';
    res.writeHead(status, out);
    Object.assign(entry, {
      status,
      bytes: noBody ? 0 : total,
      headersMs: Math.round(performance.now() - entry.t),
    });
    if (noBody) {
      res.end();
      return;
    }
    const write = (chunk) => {
      if (!res.destroyed) res.write(chunk);
    };
    res.on('close', () => shaper.cancel(write));
    for (const [i, part] of parts.entries()) {
      const until = entry.t + part.atMs - performance.now();
      if (until > 0) await new Promise((r) => setTimeout(r, until));
      if (res.destroyed) return;
      const last = i === parts.length - 1;
      await shaper.send({
        data: part.data,
        urgency: entry.urgency,
        write,
        end: last ? () => !res.destroyed && res.end() : () => {},
      });
    }
    entry.endMs = Math.round(performance.now() - entry.t);
  }

  async function handle(req, res) {
    const t = performance.now();
    const body = await readBody(req);
    const authority = req.headers[':authority'] ?? req.headers.host;
    let u;
    try {
      u = new URL(`https://${authority}${req.url}`);
    } catch {
      res.writeHead(400).end();
      return;
    }
    u.hash = '';
    const entry = {
      t,
      method: req.method,
      url: u.href,
      urgency: urgencyOf(req.headers.priority),
      priority: req.headers.priority ?? null,
    };
    log.push(entry);
    const sameOrigin = u.origin === archive.origin;

    if (sameOrigin && req.headers['sec-fetch-mode'] === 'navigate' && archive.document) {
      const doc = archive.document;
      const local = documentSource !== 'recorded' && dist && distFile(dist, '/index.html');
      const headers = { ...doc.headers };
      // Link headers of the recorded HTML name the recorded build's files.
      if (local) delete headers.link;
      let html;
      if (!local) html = recordedBody(doc.bodyRef);
      else if (documentSource === 'merged' && doc.bodyRef)
        html = Buffer.from(
          mergeDocument(recordedBody(doc.bodyRef).toString('utf8'), fs.readFileSync(local, 'utf8'))
            .html,
        );
      else html = fs.readFileSync(local);
      Object.assign(entry, { source: 'document', level: local ? documentSource : 'recorded' });
      return respond(req, res, entry, {
        status: local ? 200 : doc.status,
        headers: { 'content-type': MIME['.html'], ...headers },
        segments: documentSegments(html, doc),
        delayMs: doc.ttfbMs ?? 0,
      });
    }

    if (req.method === 'OPTIONS' && req.headers['access-control-request-method']) {
      const m = index.match({ method: 'OPTIONS', url: u.href });
      Object.assign(entry, {
        source: 'preflight',
        level: m.call ? m.level : 'synthesized',
        key: m.key,
      });
      if (m.call)
        return respond(req, res, entry, {
          status: m.call.status,
          headers: m.call.headers,
          body: null,
          delayMs: m.call.ttfbMs ?? rttMs,
        });
      return respond(req, res, entry, {
        status: 204,
        headers: {
          'access-control-allow-origin': req.headers.origin ?? '*',
          'access-control-allow-credentials': 'true',
          'access-control-allow-methods': req.headers['access-control-request-method'],
          'access-control-allow-headers': req.headers['access-control-request-headers'] ?? '*',
          'access-control-max-age': '600',
        },
        body: null,
        delayMs: rttMs,
      });
    }

    const file =
      sameOrigin && dist && (req.method === 'GET' || req.method === 'HEAD')
        ? distFile(dist, u.pathname)
        : null;
    if (file) {
      const ext = path.extname(file).toLowerCase();
      Object.assign(entry, { source: 'dist', level: 'local' });
      const ttfb = network.staticTtfbPoints
        ? sampleQuantiles(network.staticTtfbPoints, rand())
        : (network.staticTtfbMs ?? 0);
      return respond(req, res, entry, {
        status: 200,
        headers: { 'content-type': MIME[ext] ?? 'application/octet-stream' },
        body: fs.readFileSync(file),
        delayMs: ttfb,
        queueHeaders: true,
        encodeAs: COMPRESSIBLE.has(ext) ? compression : null,
        cacheKey: `${file}|${fs.statSync(file).mtimeMs}`,
      });
    }

    const m = index.match({ method: req.method, url: u.href, body });
    Object.assign(entry, { source: m.call ? 'recorded' : 'miss', level: m.level, key: m.key });
    if (!m.call) {
      return respond(req, res, entry, {
        status: 404,
        headers: { 'content-type': 'text/plain; charset=utf-8' },
        body: Buffer.from('metrics-lab replay: this request was not recorded\n'),
        delayMs: rttMs,
      });
    }
    const enc = m.call.headers['content-encoding'];
    return respond(req, res, entry, {
      status: m.call.status,
      headers: m.call.headers,
      body: recordedBody(m.call.bodyRef),
      delayMs: m.call.ttfbMs ?? rttMs,
      holdMs: m.call.downloadMs ?? 0,
      encodeAs:
        enc && DEFAULT_LEVEL[enc]
          ? compression?.encoding === enc
            ? compression
            : { encoding: enc }
          : null,
      cacheKey: m.call.bodyRef,
    });
  }

  const server = http2.createSecureServer({ ...tls, allowHTTP1: true });
  const sessions = new Set();
  server.on('session', (s) => {
    sessions.add(s);
    s.on('close', () => sessions.delete(s));
  });
  server.on('request', (req, res) => {
    handle(req, res).catch((err) => {
      if (!res.headersSent && !res.destroyed) res.writeHead(500).end(String(err.message));
    });
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  return {
    port: server.address().port,
    /**
     * Next page load: serve this dist, start call order over, empty the log.
     * The same `seed` gives the same sampled network times in the same request order.
     */
    beginRun(distDir, { seed = 0 } = {}) {
      dist = distDir;
      index.reset();
      log = [];
      rand = seededRandom(seed);
    },
    takeLog() {
      const out = log.map(({ t: _t, ...rest }) => rest);
      log = [];
      return out;
    },
    close() {
      shaper.close();
      for (const s of sessions) s.destroy();
      return new Promise((r) => server.close(() => r()));
    },
  };
}
