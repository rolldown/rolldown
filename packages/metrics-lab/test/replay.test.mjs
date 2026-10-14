import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import http2 from 'node:http2';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';

import { findChrome, launchBrowser } from '../lib/cdp.mjs';
import { criticalPath, makeRedactor } from '../lib/field/analysis.mjs';
import { timingRun } from '../lib/field/capture.mjs';
import { replayCertificate } from '../lib/replay/cert.mjs';
import {
  archiveGaps,
  bodyHashes,
  buildArchive,
  calibrate,
  calibrationVerdict,
  createResponseIndex,
  earlyFraction,
  findVolatileParams,
  fitNetworkModel,
  mergeDocument,
  normalizeBody,
  peakThroughputBps,
  requestKey,
  safeHeaders,
  sampleQuantiles,
  seededRandom,
  sha256,
} from '../lib/replay/model.mjs';
import {
  cmdInspect,
  cmdReplay,
  recordTarget,
  replayBrowserArgs,
  summarizeLog,
  validateArchive,
} from '../lib/replay/run.mjs';
import { chooseCompression, documentSegments, startReplayServer } from '../lib/replay/server.mjs';
import { createShaper, urgencyOf } from '../lib/replay/shaper.mjs';

const ORIGIN = 'https://app.example.test';

test('requestKey sorts the query and drops only the volatile parameters of that route', () => {
  assert.equal(
    requestKey('GET', `${ORIGIN}/api/items?b=2&a=1`),
    'GET app.example.test/api/items?a=1&b=2',
  );
  const volatile = { 'GET app.example.test/api/items': ['t'] };
  assert.equal(
    requestKey('GET', `${ORIGIN}/api/items?t=99&a=1`, volatile),
    'GET app.example.test/api/items?a=1',
  );
  assert.equal(
    requestKey('GET', `${ORIGIN}/api/other?t=99`, volatile),
    'GET app.example.test/api/other?t=99',
  );
  assert.equal(
    requestKey('POST', `${ORIGIN}/api/items?t=99`, volatile),
    'POST app.example.test/api/items?t=99',
  );
});

test('findVolatileParams flags values that change between runs, not pages within a run', () => {
  const run = (t) => [
    { method: 'GET', url: `${ORIGIN}/api/items?page=1&t=${t}` },
    { method: 'GET', url: `${ORIGIN}/api/items?page=2&t=${t + 1}` },
    { method: 'GET', url: `${ORIGIN}/api/fixed?x=1` },
  ];
  const once = [{ method: 'GET', url: `${ORIGIN}/api/once?r=${Math.random()}` }];
  assert.deepEqual(findVolatileParams([[...run(100), ...once], run(200)]), {
    'GET app.example.test/api/items': ['t'],
  });
});

test('normalizeBody replaces time stamps, UUIDs and epoch numbers, not small numbers', () => {
  const a = normalizeBody(
    '{"at":1760000000123,"id":"123e4567-e89b-12d3-a456-426614174000","n":5,"when":"2026-01-02T03:04:05.678Z"}',
  );
  assert.equal(a, '{"at":<n>,"id":"<uuid>","n":5,"when":"<time>"}');
  assert.deepEqual(bodyHashes(null), { bodyHash: null, bodyNormHash: null });
});

test('safeHeaders keeps how the browser treats a response, never cookies or identifiers', () => {
  const kept = safeHeaders({
    'Content-Type': 'application/json',
    'Set-Cookie': 'session=secret',
    'cf-ray': 'abc',
    Authorization: 'Bearer x',
    'Access-Control-Allow-Origin': ORIGIN,
    'content-encoding': 'br',
  });
  assert.deepEqual(kept, {
    'content-type': 'application/json',
    'access-control-allow-origin': ORIGIN,
    'content-encoding': 'br',
  });
});

test('the response index serves calls in order and names the match level', () => {
  const post = (body) => ({
    reqBodyHash: sha256(body),
    reqBodyNormHash: sha256(normalizeBody(body)),
  });
  const endpoints = {
    'GET app.example.test/api/items': {
      calls: [
        { url: `${ORIGIN}/api/items?t=1`, id: 'first' },
        { url: `${ORIGIN}/api/items?t=2`, id: 'second' },
      ],
    },
    'POST app.example.test/api/track': {
      calls: [
        { id: 'boot', ...post('{"name":"boot","at":1760000000001}') },
        { id: 'ready', ...post('{"name":"ready","at":1760000000002}') },
      ],
    },
  };
  const index = createResponseIndex(endpoints, { 'GET app.example.test/api/items': ['t'] });
  const get = (t) => index.match({ method: 'GET', url: `${ORIGIN}/api/items?t=${t}` });
  assert.deepEqual(
    [get(1), get(9), get(9)].map((m) => [m.call.id, m.level]),
    [
      ['first', 'exact'],
      ['second', 'volatile'],
      ['second', 'repeat'],
    ],
  );
  const track = (body) => index.match({ method: 'POST', url: `${ORIGIN}/api/track`, body });
  assert.deepEqual(
    [
      track('{"name":"ready","at":1760000000002}'),
      track('{"name":"boot","at":1760000099999}'),
      track('{"name":"boot","at":1760000000001}'),
    ].map((m) => [m.call.id, m.level]),
    [
      ['ready', 'body'],
      ['boot', 'body-normalized'],
      ['boot', 'repeat'],
    ],
  );
  assert.equal(index.match({ method: 'GET', url: `${ORIGIN}/api/new` }).level, 'miss');
  index.reset();
  assert.equal(get(1).call.id, 'first', 'reset starts a new page load');
});

const row = (url, o) => ({
  method: 'GET',
  url,
  type: 'Script',
  status: 200,
  failed: null,
  blocked: null,
  ...o,
});

test('fitNetworkModel reads static TTFB, RTT and the shared bandwidth from downloads', () => {
  // 4 parallel 250 kB downloads over the same 500 ms: 1 MB in 0.5 s = 2 MB/s on the link.
  const run = [0, 1, 2, 3].map((i) =>
    row(`${ORIGIN}/assets/c${i}.js`, {
      sendStartMs: 100,
      headersEndMs: 150 + i * 10,
      endMs: 650 + i * 10,
      encodedBytes: 250_000,
    }),
  );
  run.push(
    row('https://other.example.test/x.js', {
      sendStartMs: 0,
      headersEndMs: 900,
      endMs: 901,
      encodedBytes: 10,
    }),
  );
  assert.ok(Math.abs(peakThroughputBps(run, 250) - 2_000_000) / 2_000_000 < 0.05);
  const model = fitNetworkModel([run, run], ORIGIN);
  assert.equal(model.staticTtfbMs, 65);
  assert.equal(model.rttMs, 50);
  assert.ok(Math.abs(model.bandwidthBps - 2_000_000) / 2_000_000 < 0.05);
  assert.equal(
    model.estimates.staticSamples,
    8,
    'the cross-origin script is not a static file of the origin',
  );
  assert.equal(model.staticTtfbPoints.length, 21);
  assert.equal(model.staticTtfbPoints[0], 50);
  assert.equal(model.staticTtfbPoints[20], 80);
  assert.equal(model.estimates.idleLinkSamples, 8);
  assert.equal(model.wireBytes, 0, 'no loaded request, no queue to fit');
});

test('fitNetworkModel separates the idle-link TTFB from the queue a busy link adds', () => {
  // A 2 MB download over 2 s (1000 B/ms) keeps the link busy for the first 10 files.
  const run = [
    row('https://cdn.example.test/big.bin', {
      type: 'Other',
      sendStartMs: 0,
      headersEndMs: 0,
      endMs: 2000,
      encodedBytes: 2_000_000,
    }),
  ];
  for (let i = 0; i < 10; i++) {
    run.push(
      row(`${ORIGIN}/assets/busy${i}.js`, {
        sendStartMs: 100 + i * 100,
        headersEndMs: 800 + i * 100,
        endMs: 801 + i * 100,
        encodedBytes: 1000,
      }),
    );
  }
  for (let i = 0; i < 25; i++) {
    const t = 3000 + i * 400;
    run.push(
      row(`${ORIGIN}/assets/idle${i}.js`, {
        sendStartMs: t,
        headersEndMs: t + 300,
        endMs: t + 301,
        encodedBytes: 1000,
      }),
    );
  }
  const model = fitNetworkModel([run], ORIGIN);
  assert.equal(model.staticTtfbMs, 300, 'the base TTFB comes from the idle link only');
  assert.equal(model.estimates.idleLinkSamples, 25);
  assert.equal(model.estimates.loadedExtraTtfbMs, 400);
  assert.ok(
    model.wireBytes > 380_000 && model.wireBytes < 450_000,
    `400 ms of queue at ~1 MB/s (${model.wireBytes})`,
  );
});

test('sampled network times follow the recorded distribution and repeat with the seed', () => {
  const points = Array.from({ length: 21 }, (_, i) => 100 + i * 10); // p0 = 100 ... p100 = 300
  assert.equal(sampleQuantiles(points, 0), 100);
  assert.equal(sampleQuantiles(points, 0.5), 200);
  assert.equal(sampleQuantiles(points, 0.525), 205);
  assert.equal(sampleQuantiles(points, 1), 300);
  const a = seededRandom(7);
  const b = seededRandom(7);
  const draws = Array.from({ length: 5 }, () => a());
  assert.deepEqual(
    draws,
    Array.from({ length: 5 }, () => b()),
    'same seed, same draws: a pair shares its network luck',
  );
  assert.notDeepEqual(draws, Array.from({ length: 5 }, seededRandom(8)));
});

test('earlyFraction and documentSegments reproduce an early flush of the HTML head', () => {
  const doc = {
    headersEndMs: 500,
    chunks: [
      { ms: 501, bytes: 300 },
      { ms: 520, bytes: 100 },
      { ms: 760, bytes: 600 },
    ],
  };
  assert.equal(earlyFraction(doc), 0.4);
  assert.equal(earlyFraction({ headersEndMs: 500 }), null);
  const html = Buffer.from(
    '<html><head><script src="/stub.js"></script></head><body><script src="/app.js"></script></body></html>',
  );
  const parts = documentSegments(html, { ttfbMs: 500, downloadMs: 260, earlyFraction: 0.4 });
  assert.deepEqual(
    parts.map((p) => p.atMs),
    [500, 760],
  );
  assert.equal(Buffer.concat(parts.map((p) => p.data)).toString(), html.toString());
  assert.ok(parts[0].data.toString().endsWith('>'), 'the cut ends a tag');
  assert.match(parts[1].data.toString(), /app\.js/, 'the entry script comes with the late part');
  assert.equal(
    documentSegments(html, { ttfbMs: 500, downloadMs: 260, earlyFraction: null }).length,
    1,
  );
});

test('replayBrowserArgs forces the recorded network rating only when asked', () => {
  assert.ok(
    replayBrowserArgs(4000).every((a) => !a.startsWith('--force-effective-connection-type')),
  );
  assert.ok(
    replayBrowserArgs(4000, { ect: '3g' }).includes('--force-effective-connection-type=3G'),
  );
  assert.ok(replayBrowserArgs(4000)[0].includes('127.0.0.1:4000'));
  assert.ok(
    replayBrowserArgs(4000).includes('--no-proxy-server'),
    'a system proxy would resolve the hosts itself',
  );
});

test('buildArchive takes responses from the body run and median times from the timing runs', () => {
  const config = { origin: ORIGIN, gate: '/api/items', block: [], ignoreLcp: null, redact: [] };
  const variant = { name: 'base', url: `${ORIGIN}/`, build: null };
  const doc = (ttfb) =>
    row(`${ORIGIN}/`, {
      type: 'Document',
      sendStartMs: 0,
      headersEndMs: ttfb,
      endMs: ttfb + 5,
      encodedBytes: 900,
    });
  const api = (t, ttfb) =>
    row(`${ORIGIN}/api/items?t=${t}`, {
      type: 'Fetch',
      startMs: 300,
      sendStartMs: 300,
      headersEndMs: 300 + ttfb,
      endMs: 310 + ttfb,
      encodedBytes: 200,
    });
  const timing = [100, 200, 300].map((ttfb, i) => {
    const requests = [doc(ttfb), api(i, ttfb * 10)];
    const lcp = 410 + ttfb * 10;
    return {
      requests,
      lcp,
      criticalPath: criticalPath(requests, lcp, { pattern: '/api/items' }),
      path: '/',
    };
  });
  const bodyRun = {
    rows: [
      { ...doc(400), headers: { 'content-type': 'text/html' }, bodyRef: 'doc-ref' },
      {
        ...api(9, 5000),
        headers: { 'content-type': 'application/json' },
        bodyRef: 'api-ref',
        bodyBytes: 20,
      },
      row('https://blocked.example.test/x', {
        type: 'Fetch',
        failed: 'net::ERR_BLOCKED_BY_CLIENT',
        blocked: 'inspector',
        status: undefined,
      }),
    ],
  };
  const archive = buildArchive({ config, variant, timing, bodyRun, recordedAtMs: 1 });
  assert.deepEqual(archive.volatile, { 'GET app.example.test/api/items': ['t'] });
  assert.equal(archive.document.ttfbMs, 200);
  assert.equal(archive.document.bodyRef, 'doc-ref');
  const items = archive.endpoints['GET app.example.test/api/items'];
  assert.equal(items.calls.length, 1);
  assert.equal(items.calls[0].ttfbMs, 2000, 'median of the timing runs, not the body run');
  assert.equal(items.calls[0].samples, 3);
  assert.equal(Object.keys(archive.endpoints).length, 1, 'blocked requests are not replayable');
  assert.equal(archive.field.lcpMs, 2410);
  assert.equal(archive.field.ttfbMs, 2000);
  assert.deepEqual(
    archive.hosts.map((h) => [h.host, h.blocked]),
    [
      ['app.example.test', 0],
      ['blocked.example.test', 1],
    ],
  );
});

test('mergeDocument adds the host-injected tags to the local HTML, never recorded build files', () => {
  const recorded =
    '<!doctype html><html><head>' +
    '<script nonce="n1" id="config" type="application/json">{"commit":"old"}</script>' +
    '<script nonce="n1">window.__injected = 1;</script>' +
    '<link rel="modulepreload" href="/assets/vendor.Zz99Yy88.js" nonce="n1">' +
    '<script type="module" crossorigin src="/assets/app.Aa11Bb22.js" nonce="n1"></script>' +
    '<script id="sw" src="/sw/sw.Cc33Dd44.js" nonce="n1"></script>' +
    '</head><body></body></html>';
  const local =
    '<!doctype html><html><head>' +
    '<script id="config" type="application/json">{"commit":"new"}</script>' +
    '<script type="module" crossorigin src="/assets/app.Ee55Ff66.js"></script>' +
    '</head><body></body></html>';
  const { html, added } = mergeDocument(recorded, local);
  assert.equal(
    html,
    '<!doctype html><html><head>' +
      '<script id="config" type="application/json">{"commit":"new"}</script>' +
      '<script>window.__injected = 1;</script>' +
      '<script type="module" crossorigin src="/assets/app.Ee55Ff66.js"></script>' +
      '<script id="sw" src="/sw/sw.Cc33Dd44.js"></script>' +
      '</head><body></body></html>',
  );
  assert.deepEqual(added, [
    { tag: 'script', ref: null, inlineChars: 39 },
    { tag: 'script', ref: '/sw/sw.<hash>.js', inlineChars: 0 },
  ]);
  assert.deepEqual(mergeDocument(local, local), { html: local, added: [] });
});

test('calibrate passes inside max(15%, 150ms) and fails outside', () => {
  const field = { lcpMs: 2000, beforeGateMs: 1000, ttfbMs: 900 };
  assert.equal(calibrate(field, { lcpMs: 2250, beforeGateMs: 1100, ttfbMs: 1000 }).pass, true);
  const off = calibrate(field, { lcpMs: 2400, beforeGateMs: 1000, ttfbMs: 900 });
  assert.equal(off.pass, false);
  assert.equal(off.rows.find((r) => r.key === 'lcpMs').deltaMs, 400);
});

test('archiveGaps lists requests most field runs made that the archive cannot answer', () => {
  const row = (url, extra = {}) => ({ method: 'GET', url, type: 'Fetch', status: 200, ...extra });
  const archive = { volatile: {}, endpoints: { 'GET app.example.test/api/items': { calls: [] } } };
  const runs = [0, 1, 2, 3].map((i) => ({
    requests: [
      row(`${ORIGIN}/`, { type: 'Document' }),
      row(`${ORIGIN}/api/items`),
      row(`${ORIGIN}/assets/app.js`, { type: 'Script' }),
      ...(i < 3 ? [row(`${ORIGIN}/api/late?b=2&a=1`)] : []),
      row(`${ORIGIN}/cdn-cgi/check/${i}`, { method: 'POST', type: 'XHR' }),
      ...(i === 0 ? [row(`${ORIGIN}/api/once`)] : []),
      row(`${ORIGIN}/api/blocked`, { blocked: true }),
      row(`${ORIGIN}/api/failed`, { failed: true }),
    ],
  }));
  const inBuild = (r) => r.url.includes('/assets/');
  assert.deepEqual(archiveGaps(archive, runs, { inBuild }), [
    { key: 'GET app.example.test/api/late?a=1&b=2', runs: 3, of: 4 },
  ]);
  assert.equal(
    archiveGaps(archive, runs).length,
    2,
    'without inBuild, the build file counts as a gap too',
  );
  assert.deepEqual(archiveGaps(archive, []), []);

  assert.equal(calibrationVerdict({ pass: true }, []), 'pass');
  assert.equal(calibrationVerdict({ pass: false }, []), 'model-error');
  assert.equal(
    calibrationVerdict({ pass: true }, [{ key: 'x' }]),
    'record-again',
    'a gap comes first: the rows may be right by chance',
  );
});

function fakeClock() {
  let t = 0;
  const timers = [];
  return {
    now: () => t,
    schedule: (fn, ms) => {
      timers.push({ at: t + ms, fn });
      timers.sort((a, b) => a.at - b.at);
    },
    // Microtasks (promise callbacks) run between timers, as on a real event loop.
    async run() {
      while (timers.length) {
        const next = timers.shift();
        t = next.at;
        next.fn();
        await Promise.resolve();
      }
    },
  };
}

test('the shaper shares one bandwidth between streams and sends urgent streams first', async () => {
  assert.equal(urgencyOf('u=1, i'), 1);
  assert.equal(urgencyOf(undefined), 3);
  const clock = fakeClock();
  const shaper = createShaper({
    bytesPerSec: 1000,
    tickMs: 10,
    now: clock.now,
    schedule: clock.schedule,
  });
  const ends = {};
  const stream = (name, size, urgency) =>
    shaper.send({
      data: Buffer.alloc(size),
      urgency,
      write: () => {},
      end: () => (ends[name] = clock.now()),
    });
  const all = Promise.all([stream('a', 300, 3), stream('b', 300, 3), stream('urgent', 100, 1)]);
  await clock.run();
  await all;
  assert.equal(ends.urgent, 100);
  assert.equal(ends.a, 400);
  assert.equal(
    ends.b,
    700,
    'two parallel 300 B streams at 1000 B/s take 0.6 s together, not 0.3 s',
  );

  const writes = [];
  const unlimited = createShaper({ bytesPerSec: Infinity });
  await unlimited.send({
    data: Buffer.from('abc'),
    write: (c) => writes.push(c.toString()),
    end: () => writes.push('end'),
  });
  assert.deepEqual(writes, ['abc', 'end']);
});

test('with a wire queue, response headers wait behind the bytes already on the wire', async () => {
  const clock = fakeClock();
  const shaper = createShaper({
    bytesPerSec: 1000,
    wireBytes: 500,
    tickMs: 10,
    now: clock.now,
    schedule: clock.schedule,
  });
  const at = {};
  // 105 ms, not 100: a tie with a 10 ms tick would make the result depend on timer order.
  clock.schedule(() => shaper.header().then(() => (at.header = clock.now())), 105);
  const big = shaper.send({
    data: Buffer.alloc(2000),
    urgency: 3,
    write: () => {},
    end: () => (at.big = clock.now()),
  });
  await clock.run();
  await big;
  assert.equal(
    at.header,
    590,
    'headers asked for at 105 ms wait for the 490 bytes the 100 ms tick left on the wire, at 1000 B/s',
  );
  assert.equal(at.big, 2000);
  assert.equal(
    await shaper.header().then(() => 'now'),
    'now',
    'an empty wire lets headers through at once',
  );
});

// --- server ------------------------------------------------------------------------

function syntheticSite() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'replay-test-'));
  const dist = path.join(dir, 'dist');
  const archiveDir = path.join(dir, 'archive');
  fs.mkdirSync(path.join(dist, 'assets'), { recursive: true });
  fs.mkdirSync(path.join(archiveDir, 'bodies'), { recursive: true });
  fs.writeFileSync(
    path.join(dist, 'index.html'),
    '<!doctype html><div id="splash">loading</div><script type="module" src="/assets/app.js"></script><body></body>',
  );
  fs.writeFileSync(
    path.join(dist, 'assets', 'app.js'),
    `const items = await (await fetch('/api/items?t=' + Date.now())).json();\n` +
      `const lib = await (await fetch('https://cdn.example.test/lib.json')).json();\n` +
      `await fetch('/api/track', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: 'boot', at: Date.now() }) });\n` +
      `const h = document.createElement('h1');\n` +
      `h.textContent = items.title + ' ' + lib.name + ' ' + 'x'.repeat(200);\n` +
      `document.body.append(h);\n`,
  );
  const store = (text) => {
    const ref = sha256(text);
    fs.writeFileSync(path.join(archiveDir, 'bodies', ref), text);
    return ref;
  };
  const track = '{"name":"boot","at":1760000000001}';
  const archive = {
    schemaVersion: 1,
    kind: 'metrics-lab-replay',
    recordedAtMs: Date.now(),
    url: `${ORIGIN}/`,
    origin: ORIGIN,
    variant: 'base',
    build: null,
    gate: '/api/items',
    block: [],
    ignoreLcp: '#splash',
    redact: [],
    volatile: { 'GET app.example.test/api/items': ['t'] },
    document: {
      url: `${ORIGIN}/`,
      status: 200,
      headers: {
        'content-type': 'text/html; charset=utf-8',
        link: '</assets/old.js>; rel=modulepreload',
      },
      bodyRef: store('<!doctype html><title>recorded</title>'),
      ttfbMs: 120,
    },
    endpoints: {
      'GET app.example.test/api/items': {
        method: 'GET',
        calls: [
          {
            url: `${ORIGIN}/api/items?t=1`,
            status: 200,
            headers: { 'content-type': 'application/json', 'content-encoding': 'br' },
            bodyRef: store('{"title":"Synthetic items"}'),
            ttfbMs: 300,
          },
        ],
      },
      'GET cdn.example.test/lib.json': {
        method: 'GET',
        calls: [
          {
            url: 'https://cdn.example.test/lib.json',
            status: 200,
            headers: { 'content-type': 'application/json', 'access-control-allow-origin': ORIGIN },
            bodyRef: store('{"name":"lib"}'),
            ttfbMs: 80,
          },
        ],
      },
      'POST app.example.test/api/track': {
        method: 'POST',
        calls: [
          {
            url: `${ORIGIN}/api/track`,
            status: 204,
            headers: {},
            bodyRef: null,
            ttfbMs: 50,
            reqBodyHash: sha256(track),
            reqBodyNormHash: sha256(normalizeBody(track)),
          },
        ],
      },
    },
    assets: [{ path: '/assets/app.js', encodedBytes: 400, encoding: 'br' }],
    network: { staticTtfbMs: 40, rttMs: 20, bandwidthBps: 2_000_000, estimates: {} },
    field: {
      runs: 3,
      paths: ['/'],
      lcpMs: 700,
      documentEndMs: 125,
      beforeGateMs: 220,
      gateAfterDocumentMs: 95,
      ttfbMs: 300,
      downloadMs: 1,
      afterGateMs: 150,
    },
    hosts: [],
    page: null,
  };
  fs.writeFileSync(path.join(archiveDir, 'archive.json'), JSON.stringify(archive));
  return { dir, dist, archiveDir, archive };
}

function h2request(client, headers, body = null) {
  return new Promise((resolve, reject) => {
    const t0 = performance.now();
    const req = client.request(headers);
    const chunks = [];
    let head = null;
    req.on('response', (h) => {
      head = { status: h[':status'], headers: h, headersMs: performance.now() - t0 };
    });
    req.on('data', (c) => chunks.push(c));
    req.on('end', () =>
      resolve({ ...head, body: Buffer.concat(chunks), endMs: performance.now() - t0 }),
    );
    req.on('error', reject);
    req.end(body ?? undefined);
  });
}

test('the replay server routes document, dist, recorded, preflight and unrecorded requests', async (t) => {
  const site = syntheticSite();
  const compression = chooseCompression(site.archive.assets, site.dist);
  assert.equal(compression.encoding, null, 'assets under 20 kB do not calibrate compression');
  const server = await startReplayServer({
    archive: site.archive,
    archiveDir: site.archiveDir,
    tls: replayCertificate(path.join(site.dir, 'cert')),
    network: site.archive.network,
    compression: { encoding: 'br', level: 5 },
  });
  const client = http2.connect(`https://127.0.0.1:${server.port}`, { rejectUnauthorized: false });
  t.after(async () => {
    client.close();
    await server.close();
    fs.rmSync(site.dir, { recursive: true, force: true });
  });
  server.beginRun(site.dist);
  const app = { ':authority': 'app.example.test', 'accept-encoding': 'br, gzip' };

  const doc = await h2request(client, { ...app, ':path': '/', 'sec-fetch-mode': 'navigate' });
  assert.equal(doc.status, 200);
  assert.ok(doc.headersMs >= 110, `document waits for the recorded TTFB (${doc.headersMs}ms)`);
  assert.equal(doc.headers.link, undefined, 'recorded Link headers name the recorded build');
  assert.equal(
    doc.headers['content-encoding'],
    undefined,
    'the document goes uncompressed, so its parts decode at once',
  );
  assert.match(
    doc.body.toString(),
    /assets\/app\.js/,
    'the local index.html, not the recorded one',
  );

  const js = await h2request(client, { ...app, ':path': '/assets/app.js', priority: 'u=1' });
  assert.equal(js.headers['content-encoding'], 'br');
  assert.ok(
    js.headersMs >= 35 && js.headersMs < 250,
    `static TTFB from the model (${js.headersMs}ms)`,
  );

  const items = await h2request(client, { ...app, ':path': '/api/items?t=12345' });
  assert.ok(items.headersMs >= 290, `recorded server time (${items.headersMs}ms)`);
  assert.equal(JSON.parse(zlib.brotliDecompressSync(items.body)).title, 'Synthetic items');

  const tracked = await h2request(
    client,
    { ...app, ':path': '/api/track', ':method': 'POST' },
    '{"name":"boot","at":1760000055555}',
  );
  assert.equal(tracked.status, 204);

  const pre = await h2request(client, {
    ':authority': 'cdn.example.test',
    ':path': '/lib.json',
    ':method': 'OPTIONS',
    origin: ORIGIN,
    'access-control-request-method': 'GET',
    'access-control-request-headers': 'x-custom',
  });
  assert.equal(pre.status, 204);
  assert.equal(pre.headers['access-control-allow-origin'], ORIGIN);

  const miss = await h2request(client, { ...app, ':path': '/api/new' });
  assert.equal(miss.status, 404);

  const log = server.takeLog();
  assert.deepEqual(
    log.map((e) => `${e.source}:${e.level}`),
    [
      'document:merged',
      'dist:local',
      'recorded:volatile',
      'recorded:body-normalized',
      'preflight:synthesized',
      'miss:miss',
    ],
  );
  assert.equal(log[1].urgency, 1);
});

test('recordTarget builds a target from flags alone and never repeats a cookie value', () => {
  const { config, variant } = recordTarget({
    url: `${ORIGIN}/home`,
    cookie: ['version=canary-123'],
    block: ['analytics.example.test'],
    gate: '/api/items',
    'ignore-lcp': '#splash',
    build: '/builds/served',
  });
  assert.equal(variant.name, 'recorded');
  assert.equal(variant.url, `${ORIGIN}/home`);
  assert.deepEqual(variant.cookies, [{ name: 'version', value: 'canary-123' }]);
  assert.equal(variant.build, '/builds/served');
  assert.deepEqual(
    [config.origin, config.gate, config.ignoreLcp, config.block],
    [ORIGIN, '/api/items', '#splash', ['analytics.example.test']],
  );
  assert.throws(
    () => recordTarget({ url: ORIGIN, cookie: ['secretvalue'] }),
    (err) => /--cookie needs name=value/.test(err.message),
  );
  assert.throws(() => recordTarget({}), /--url/);
});

test('validateArchive rejects an incomplete archive and warns about a less faithful one', () => {
  const site = syntheticSite();
  try {
    const ok = validateArchive(site.archive, site.archiveDir);
    assert.deepEqual(ok.fatal, []);
    assert.ok(ok.warnings.some((w) => /Chrome network rating/.test(w)));
    fs.rmSync(path.join(site.archiveDir, 'bodies', site.archive.document.bodyRef));
    assert.match(
      validateArchive(site.archive, site.archiveDir).fatal.join(),
      /1 of 3 response bodies are missing/,
    );
    assert.match(
      validateArchive({ ...site.archive, schemaVersion: 99 }, site.archiveDir).fatal.join(),
      /record again/,
    );
  } finally {
    fs.rmSync(site.dir, { recursive: true, force: true });
  }
});

test('inspect reports a gap from the field runs, and a build file is not one', async () => {
  const site = syntheticSite();
  const logLine = console.log;
  console.log = () => {};
  try {
    const row = (url, type = 'Fetch') => ({ method: 'GET', url, type, status: 200 });
    const runs = [0, 1].map(() => ({
      requests: [
        row(`${ORIGIN}/api/items?t=5`),
        row(`${ORIGIN}/api/late`),
        row(`${ORIGIN}/assets/app.js`, 'Script'),
      ],
    }));
    fs.writeFileSync(path.join(site.archiveDir, 'timing.json'), JSON.stringify({ runs }));
    fs.writeFileSync(
      path.join(site.archiveDir, 'archive.json'),
      JSON.stringify({ ...site.archive, build: site.dist }),
    );
    const result = await cmdInspect(['--archive', site.archiveDir]);
    assert.equal(result.usable, true);
    assert.equal(result.complete, false);
    assert.deepEqual(result.gaps, [{ key: 'GET app.example.test/api/late', runs: 2, of: 2 }]);
  } finally {
    console.log = logLine;
    fs.rmSync(site.dir, { recursive: true, force: true });
  }
});

test('inspect lists late static chunks of the field runs from the build manifest', async () => {
  const site = syntheticSite();
  const logLine = console.log;
  console.log = () => {};
  try {
    fs.writeFileSync(path.join(site.dist, 'assets', 'late.js'), '');
    const manifest = path.join(site.dist, '.vite', 'manifest.json');
    fs.mkdirSync(path.dirname(manifest));
    fs.writeFileSync(
      manifest,
      JSON.stringify({
        app: { file: 'assets/app.js', src: 'src/app.ts', imports: ['late'] },
        late: { file: 'assets/late.js', src: 'src/late.ts' },
      }),
    );
    const script = (file, startMs, endMs) => ({
      method: 'GET',
      url: `${ORIGIN}/assets/${file}.js`,
      type: 'Script',
      status: 200,
      startMs,
      endMs,
      encodedBytes: 2048,
    });
    // The archive RTT is 20 ms: late.js starts 300 ms after app.js finished.
    const runs = [0, 1].map(() => ({
      lcp: 900,
      requests: [script('app', 50, 100), script('late', 400, 450)],
    }));
    fs.writeFileSync(path.join(site.archiveDir, 'timing.json'), JSON.stringify({ runs }));

    const none = await cmdInspect(['--archive', site.archiveDir]);
    assert.equal(none.lateChunksBeforeLcp.state, 'unknown');
    assert.match(none.lateChunksBeforeLcp.reason, /no local copy of the recorded build/);

    fs.writeFileSync(
      path.join(site.archiveDir, 'archive.json'),
      JSON.stringify({ ...site.archive, build: site.dist }),
    );
    const found = await cmdInspect(['--archive', site.archiveDir]);
    assert.deepEqual(found.lateChunksBeforeLcp, {
      state: 'open',
      reason: null,
      rttMs: 20,
      groups: [
        {
          parent: 'assets/app.js',
          chunks: 1,
          bytes: 2048,
          maxDelayMs: 300,
          runs: '2/2',
          importChain: ['assets/app.js', 'assets/late.js'],
          source: 'src/late.ts',
        },
      ],
    });

    const later = new Date(Date.now() + 10_000);
    fs.utimesSync(path.join(site.dist, 'assets', 'late.js'), later, later);
    const stale = await cmdInspect(['--archive', site.archiveDir]);
    assert.equal(stale.lateChunksBeforeLcp.state, 'unknown');
    assert.match(
      stale.lateChunksBeforeLcp.reason,
      /older than files of the build; copy a build with mv or cp -p/,
    );
  } finally {
    console.log = logLine;
    fs.rmSync(site.dir, { recursive: true, force: true });
  }
});

test('replay.mjs --json gives a second agent one result object on stdout', () => {
  const site = syntheticSite();
  const cli = fileURLToPath(new URL('../replay.mjs', import.meta.url));
  try {
    const ok = spawnSync(
      process.execPath,
      [cli, 'inspect', '--archive', site.archiveDir, '--json'],
      { encoding: 'utf8' },
    );
    assert.equal(ok.status, 0, ok.stderr);
    const lines = ok.stdout.trim().split('\n');
    assert.equal(lines.length, 1, 'stdout holds the result only; the log goes to stderr');
    const result = JSON.parse(lines[0]);
    assert.equal(result.usable, true);
    assert.equal(result.complete, true);
    assert.deepEqual(result.gaps, []);
    assert.equal(result.requestKeys, 3);
    assert.match(result.next.calibrate, /replay\.mjs replay --archive .* --calibrate/);
    assert.match(ok.stderr, /archive:/);

    const bad = spawnSync(
      process.execPath,
      [cli, 'inspect', '--archive', path.join(site.dir, 'nope'), '--json'],
      { encoding: 'utf8' },
    );
    assert.equal(bad.status, 2);
    assert.match(JSON.parse(bad.stdout).error, /no replay archive/);
  } finally {
    fs.rmSync(site.dir, { recursive: true, force: true });
  }
});

test('replay: headless Chrome loads the real origin from the local server and calibrates', async (t) => {
  try {
    findChrome();
  } catch {
    t.skip('Chrome/Edge is not installed');
    return;
  }
  const site = syntheticSite();
  const server = await startReplayServer({
    archive: site.archive,
    archiveDir: site.archiveDir,
    tls: replayCertificate(path.join(site.dir, 'cert')),
    network: site.archive.network,
  });
  const browser = await launchBrowser({
    profileDir: path.join(site.dir, 'chrome'),
    extraArgs: replayBrowserArgs(server.port),
  });
  t.after(async () => {
    await browser.close();
    await server.close();
    fs.rmSync(site.dir, { recursive: true, force: true });
  });
  const config = {
    url: site.archive.url,
    origin: ORIGIN,
    block: [],
    ignoreLcp: '#splash',
    gate: '/api/items',
  };
  server.beginRun(site.dist);
  const { browserContextId } = await browser.cdp.send('Target.createBrowserContext');
  const redact = makeRedactor();
  const run = await timingRun(browser.cdp, {
    config,
    variant: { name: 'base', url: site.archive.url, cookies: [] },
    context: browserContextId,
    saved: new Map(),
    redact,
    settleMs: 200,
    timeoutMs: 15_000,
    throttle: null,
  });
  const log = server.takeLog();
  assert.equal(run.invalid, null);
  const doc = run.requests.find((r) => r.type === 'Document');
  assert.equal(doc.url, `${ORIGIN}/`, 'the page runs at the recorded origin');
  assert.equal(doc.protocol, 'h2');
  assert.ok(doc.headersEndMs - doc.sendStartMs >= 110);
  const cp = criticalPath(run.requests, run.sample.lcp, { pattern: config.gate });
  assert.ok(
    cp.breakdown.ttfbMs >= 290,
    `gate TTFB is the recorded server time (${cp.breakdown.ttfbMs}ms)`,
  );
  const info = summarizeLog(log, [{ ...run, criticalPath: cp }], redact, ['/']);
  assert.deepEqual(
    info.unrecorded.filter((u) => u.byPage),
    [],
  );
  assert.ok(
    info.levels['recorded:body-normalized'] === 1 && info.levels['recorded:exact'] === 1,
    JSON.stringify(info.levels),
  );

  // The command end to end: one pair, calibration in the report and the result.
  const manifest = path.join(site.dist, '.vite', 'manifest.json');
  fs.mkdirSync(path.dirname(manifest));
  fs.writeFileSync(manifest, JSON.stringify({ app: { file: 'assets/app.js', src: 'src/app.ts' } }));
  const out = path.join(site.dir, 'report');
  const logLine = console.log;
  console.log = () => {};
  let result;
  try {
    result = await cmdReplay([
      '--archive',
      site.archiveDir,
      '--dist',
      site.dist,
      '--runs',
      '1',
      '--calibrate',
      '--settle',
      '200',
      '--pause',
      '0',
      '--out',
      out,
    ]);
  } finally {
    console.log = logLine;
  }
  const report = JSON.parse(fs.readFileSync(path.join(out, 'replay-report.json'), 'utf8'));
  assert.equal(report.mode, 'replay');
  assert.equal(report.calibration.rows.find((r) => r.key === 'ttfbMs').pass, true);
  assert.ok(fs.existsSync(path.join(out, 'base', 'runtime-metrics.json')));
  assert.equal(result.report, path.join(out, 'replay-report.json'));
  assert.equal(result.storage, 'empty', 'the recorded page had no storage');
  assert.deepEqual(result.variants.base.unrecordedPageRequests, []);
  assert.equal(typeof result.variants.base.lcpMs, 'number');
  assert.equal(result.calibration.rows.length, 6);
  assert.deepEqual(result.calibration.gaps, []);
  assert.equal(result.calibration.verdict, result.calibration.pass ? 'pass' : 'model-error');
  // Chunk discovery reads the replay's CDP requests: app.js matches the manifest.
  assert.deepEqual(result.variants.base.lateChunksBeforeLcp, {
    state: 'clear',
    reason: null,
    rttMs: 20,
    groups: [],
  });
  const runtime = JSON.parse(
    fs.readFileSync(path.join(out, 'base', 'runtime-metrics.json'), 'utf8'),
  );
  assert.equal(runtime.chunkDiscovery.lcp.matchedScripts, 1);
});

test('replay --storage warm lets the build start from the storage its first load wrote', async (t) => {
  try {
    findChrome();
  } catch {
    t.skip('Chrome/Edge is not installed');
    return;
  }
  const site = syntheticSite();
  t.after(() => fs.rmSync(site.dir, { recursive: true, force: true }));
  // Like a boot payload cache: with a stored copy the page renders at once and
  // refreshes the copy in the background; without one it waits for the API.
  fs.writeFileSync(
    path.join(site.dist, 'assets', 'app.js'),
    `const cached = localStorage.getItem('items');\n` +
      `const live = fetch('/api/items?t=' + Date.now()).then((r) => r.json()).then((j) => { localStorage.setItem('items', JSON.stringify(j)); return j; });\n` +
      `const items = cached ? JSON.parse(cached) : await live;\n` +
      `const h = document.createElement('h1');\n` +
      `h.textContent = items.title + ' ' + 'x'.repeat(200);\n` +
      `document.body.append(h);\n`,
  );
  const archive = structuredClone(site.archive);
  archive.endpoints['GET app.example.test/api/items'].calls[0].ttfbMs = 1500;
  fs.writeFileSync(path.join(site.archiveDir, 'archive.json'), JSON.stringify(archive));

  const lcpWith = async (storage) => {
    const logLine = console.log;
    console.log = () => {};
    try {
      const out = path.join(site.dir, `report-${storage}`);
      const result = await cmdReplay([
        '--archive',
        site.archiveDir,
        '--dist',
        site.dist,
        '--runs',
        '1',
        '--storage',
        storage,
        '--settle',
        '200',
        '--pause',
        '0',
        '--out',
        out,
      ]);
      assert.equal(result.storage, storage);
      return result.variants.base.lcpMs;
    } finally {
      console.log = logLine;
    }
  };
  const empty = await lcpWith('empty');
  const warm = await lcpWith('warm');
  assert.ok(empty >= 1500, `empty storage waits for the 1500 ms API (LCP ${empty} ms)`);
  assert.ok(warm < 1000, `warm storage renders from the stored copy (LCP ${warm} ms)`);
});
