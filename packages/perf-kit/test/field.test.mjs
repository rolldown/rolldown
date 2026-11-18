import { test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { findChrome, launchBrowser } from '../lib/cdp.mjs';
import { startServer } from '../lib/serve.mjs';
import {
  chunkCoverage,
  criticalInitiatorUrl,
  criticalPath,
  findGate,
  initiatorChain,
  makeRedactor,
  outlierIndexes,
  pairedDelta,
} from '../lib/field/analysis.mjs';
import { openFieldPage } from '../lib/field/browser.mjs';
import { indexBuild, matchScript, matchSummary } from '../lib/field/build-match.mjs';
import { validateFieldConfig } from '../lib/field/config.mjs';
import { diagnosticRun, recordFieldNetwork, timingRun } from '../lib/field/capture.mjs';

test('redaction removes identifiers but keeps chunk names and query keys', () => {
  const redact = makeRedactor(['tenant-[a-z]+']);
  const url =
    'https://x.test/api/accounts/0123456789abcdef0123456789abcdef/zones?id=00000000-0000-4000-8000-000000000000&per_page=50&u=a.b@example.com&t=1791867930&org=tenant-acme';
  assert.equal(
    redact(url),
    'https://x.test/api/accounts/<hex>/zones?id=<uuid>&per_page=50&u=<email>&t=<n>&org=<redacted>',
  );
  assert.equal(redact('/assets/app-init.Ab12Cd34.js'), '/assets/app-init.Ab12Cd34.js');
  assert.equal(redact(null), null);
  assert.equal(redact('/users/a.b%40example.com/items'), '/users/<email>/items');
  assert.equal(redact('?u=first%2Blast%40example.com&x=1'), '?u=<email>&x=1');
});

const req = (over) => ({
  url: 'https://x.test/a',
  method: 'GET',
  type: 'Fetch',
  startMs: 0,
  sendStartMs: 0,
  headersEndMs: 0,
  endMs: 0,
  failed: null,
  initiator: { type: 'script', url: null, frames: [] },
  ...over,
});

const REQUESTS = [
  req({
    url: 'https://x.test/',
    type: 'Document',
    startMs: 0,
    endMs: 100,
    initiator: { type: 'other', url: null },
  }),
  req({
    url: 'https://x.test/app.js',
    type: 'Script',
    startMs: 110,
    endMs: 200,
    initiator: { type: 'parser', url: 'https://x.test/' },
  }),
  req({
    url: 'https://x.test/core.js',
    type: 'Script',
    startMs: 210,
    endMs: 600,
    initiator: { type: 'script', url: 'https://x.test/app.js' },
  }),
  req({
    url: 'https://x.test/api/items',
    startMs: 700,
    sendStartMs: 702,
    headersEndMs: 1700,
    endMs: 1750,
    initiator: { type: 'script', url: 'https://x.test/core.js' },
  }),
  req({
    url: 'https://x.test/api/flags',
    startMs: 650,
    sendStartMs: 651,
    headersEndMs: 800,
    endMs: 810,
  }),
  req({ url: 'https://x.test/api/late', startMs: 1900, endMs: 2500 }),
];

test('findGate picks the last data response before LCP, or the pinned pattern', () => {
  assert.equal(findGate(REQUESTS, 1800).gate.url, 'https://x.test/api/items');
  assert.equal(
    findGate(REQUESTS, 1800, { pattern: '/api/flags' }).gate.url,
    'https://x.test/api/flags',
  );
  const none = findGate(REQUESTS, 1800, { pattern: '/api/nothing' });
  assert.equal(none.gate, null);
  assert.match(none.reason, /no request matching/);
  assert.equal(findGate(REQUESTS, null).reason, 'no LCP');
});

test('page time comes from the document of the navigation entry, also after a client-side redirect', () => {
  const handlers = new Map();
  const net = recordFieldNetwork({
    on: (method, handler) => (handlers.set(method, handler), () => handlers.delete(method)),
  });
  const emit = (method, event) => handlers.get(method)(event);
  for (const [requestId, url, ts] of [
    ['1', 'https://x.test/', 10],
    ['2', 'https://x.test/home', 11],
  ]) {
    emit('Network.requestWillBeSent', {
      requestId,
      timestamp: ts,
      type: 'Document',
      request: { url, method: 'GET' },
      initiator: { type: 'other' },
    });
    emit('Network.responseReceived', {
      requestId,
      type: 'Document',
      response: { status: 200, timing: { requestTime: ts, sendStart: 1, receiveHeadersEnd: 50 } },
    });
    emit('Network.loadingFinished', { requestId, timestamp: ts + 0.1, encodedDataLength: 1000 });
  }
  // The navigation entry of /home: requestStart 1 ms after its timeOrigin.
  const [first, home] = net.finish(1, (v) => v, 'https://x.test/home');
  assert.deepEqual(
    [home.navigation, home.startMs, home.sendStartMs, home.endMs],
    [true, 0, 1, 100],
  );
  assert.equal(first.navigation, undefined);
  assert.equal(first.endMs, -900);
  const redirected = [
    req({ url: 'https://x.test/', type: 'Document', startMs: -1000, endMs: -900 }),
    { ...REQUESTS[0], url: 'https://x.test/home', navigation: true },
    ...REQUESTS.slice(1),
  ];
  assert.equal(criticalPath(redirected, 1800).breakdown.documentEndMs, 100);
});

test('criticalPath splits LCP at the gate and walks the initiator chain to the document', () => {
  const result = criticalPath(REQUESTS, 1800);
  assert.deepEqual(result.breakdown, {
    documentEndMs: 100,
    beforeGateMs: 700,
    gateAfterDocumentMs: 600,
    ttfbMs: 998,
    downloadMs: 50,
    afterGateMs: 50,
  });
  assert.deepEqual(
    result.chain.map((s) => s.url),
    ['https://x.test/core.js', 'https://x.test/app.js', 'https://x.test/'],
  );
  assert.deepEqual(initiatorChain(REQUESTS, 'https://x.test/missing.js')[0].type, 'unknown');
});

test('the chain follows the stack script that loaded last, not a fetch wrapper on top', () => {
  const requests = [
    ...REQUESTS.slice(0, 3),
    req({
      url: 'https://x.test/client.js',
      type: 'Script',
      startMs: 620,
      endMs: 900,
      initiator: { type: 'script', url: 'https://x.test/core.js' },
    }),
    req({
      url: 'https://x.test/api/items',
      startMs: 950,
      initiator: {
        type: 'script',
        url: 'https://x.test/core.js',
        frames: [
          { url: 'https://x.test/core.js' },
          { url: 'https://x.test/client.js' },
          { url: 'https://x.test/app.js' },
        ],
      },
    }),
  ];
  assert.equal(criticalInitiatorUrl(requests[4], requests), 'https://x.test/client.js');
  assert.deepEqual(
    initiatorChain(requests, 'https://x.test/client.js').map((s) => new URL(s.url).pathname),
    ['/client.js', '/core.js', '/app.js', '/'],
  );
});

test('pairedDelta is deterministic and counts candidate wins', () => {
  const pairs = [
    [3000, 2800],
    [3100, 3000],
    [2900, 2950],
    [3500, 3000],
    [3000, 2700],
  ];
  const a = pairedDelta(pairs);
  const b = pairedDelta(pairs);
  assert.deepEqual(a, b);
  assert.equal(a.pairs, 5);
  assert.equal(a.medianDeltaMs, -200);
  assert.equal(a.candidateFaster, 4);
  assert.ok(a.ci90Ms[0] <= a.medianDeltaMs && a.medianDeltaMs <= a.ci90Ms[1]);
  assert.equal(pairedDelta([[null, 1]]), null);
});

test('outlierIndexes flags values above 3x the median only with n >= 3', () => {
  assert.deepEqual(outlierIndexes([1000, 1100, 900, 4000]), [3]);
  assert.deepEqual(outlierIndexes([1000, 4000]), []);
});

test('chunkCoverage counts executed bytes at the cut and by settle without a sourcemap', () => {
  const fn = (ranges) => [{ functionName: '', isBlockCoverage: true, ranges }];
  const result = chunkCoverage({
    bytes: 1000,
    atCut: fn([
      { startOffset: 0, endOffset: 1000, count: 1 },
      { startOffset: 200, endOffset: 1000, count: 0 },
    ]),
    atSettle: fn([{ startOffset: 500, endOffset: 700, count: 1 }]),
  });
  assert.deepEqual(result, { bytes: 1000, cutBytes: 200, settleBytes: 400, cutRatio: 0.2 });
  assert.equal(chunkCoverage({ bytes: 10, atCut: null, atSettle: null }).cutBytes, 0);
});

test('build matching decides identity by content, not by name', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'field-build-'));
  fs.mkdirSync(path.join(dir, 'assets'));
  fs.writeFileSync(path.join(dir, 'assets', 'a.js'), 'export const a = 1;');
  fs.writeFileSync(path.join(dir, 'assets', 'b.js'), 'export const b = 2;');
  const debugId = crypto.randomUUID();
  fs.writeFileSync(
    path.join(dir, 'assets', 'c.js'),
    `export const c = 3;\n//# debugId=${debugId}\n//# sourceMappingURL=c.js.map\n`,
  );
  try {
    const index = indexBuild(dir);
    assert.equal(
      matchScript(index, { file: 'assets/a.js', body: 'export const a = 1;' }).status,
      'exact',
    );
    assert.deepEqual(matchScript(index, { file: 'assets/x.js', body: 'export const b = 2;' }), {
      status: 'renamed',
      local: 'assets/b.js',
    });
    assert.equal(
      matchScript(index, { file: 'assets/a.js', body: 'export const a = 9;' }).status,
      'different-build',
    );
    assert.equal(matchScript(index, { file: 'assets/z.js', body: 'nope' }).status, 'not-in-build');
    assert.equal(
      matchScript(index, {
        file: 'assets/c.js',
        body: `minified differently\n//# debugId=${debugId}\n//# sourceMappingURL=c.js.map\n`,
      }).status,
      'debug-id',
    );
    assert.equal(matchScript(null, { file: 'a.js', body: '' }).status, 'no-build');
    assert.deepEqual(
      matchSummary([
        { status: 'exact', bytes: 10 },
        { status: 'different-build', bytes: 30 },
      ]),
      {
        scripts: 2,
        bytes: 40,
        matchedScripts: 1,
        matchedBytes: 10,
        byStatus: { exact: 1, 'different-build': 1 },
      },
    );
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('each run starts from the saved cookies, and a restore removes every variant scope', async () => {
  const calls = [];
  const cdp = {
    send: async (method, params) => {
      calls.push({ method, params });
      if (method === 'Target.createTarget') return { targetId: 't1' };
      if (method === 'Target.attachToTarget') return { sessionId: 's1' };
      return {};
    },
    on: () => () => {},
  };
  const origin = 'https://app.example.test';
  const saved = new Map([
    [
      'v',
      {
        name: 'v',
        value: 'mine',
        domain: 'app.example.test',
        path: '/',
        secure: true,
        httpOnly: false,
        session: true,
      },
    ],
    ['flag', null],
  ]);
  const configured = [
    { name: 'v', value: 'a' },
    { name: 'v', value: 'b', domain: '.example.test' },
    { name: 'flag', value: 'on' },
  ];
  await openFieldPage(cdp, { origin, cookies: [configured[1]], saved, configured });
  const reset = calls.findIndex((c) => c.method === 'Storage.setCookies');
  const set = calls.findIndex((c) => c.method === 'Network.setCookie');
  assert.ok(reset >= 0 && reset < set, 'the reset runs before the variant cookie is set');
  assert.deepEqual(
    calls[reset].params.cookies.map((c) => [c.name, c.value, c.domain ?? c.url, c.path, c.expires]),
    [
      ['v', 'mine', 'app.example.test', '/', undefined],
      ['v', '', '.example.test', '/', 1],
      ['flag', '', origin, '/', 1],
    ],
  );
  assert.deepEqual(calls[set].params, {
    url: origin,
    secure: true,
    name: 'v',
    value: 'b',
    domain: '.example.test',
    path: '/',
  });
});

test('field config applies variant queries, resolves builds and rejects bad input', () => {
  const config = validateFieldConfig(
    {
      url: 'https://x.test/home',
      variants: {
        base: { cookies: [{ name: 'v', value: 'stable' }], build: 'dist' },
        cand: { query: { preview: 'pr-1' } },
      },
      gate: '/api/items',
    },
    '/work',
  );
  assert.equal(config.origin, 'https://x.test');
  assert.equal(config.variants.cand.url, 'https://x.test/home?preview=pr-1');
  assert.equal(config.variants.base.build, path.resolve('/work', 'dist'));
  assert.throws(() => validateFieldConfig({ url: 'nope', variants: { a: {} } }), /absolute URL/);
  assert.throws(() => validateFieldConfig({ url: 'https://x.test', variants: {} }), /at least one/);
  assert.throws(
    () =>
      validateFieldConfig({ url: 'https://x.test', variants: { a: { cookies: [{ name: 'v' }] } } }),
    /without name\/value/,
  );
});

test('field capture finds the gate, its script chain, and matches served chunks to the build', async (t) => {
  try {
    findChrome();
  } catch {
    t.skip('Chrome/Edge is not installed');
    return;
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'field-capture-'));
  fs.mkdirSync(path.join(dir, 'api'));
  fs.writeFileSync(
    path.join(dir, 'api', 'items.json'),
    JSON.stringify({ title: 'Synthetic items' }),
  );
  fs.writeFileSync(
    path.join(dir, 'core.js'),
    'export async function boot() {\n' +
      "  const res = await fetch('/api/items.json');\n" +
      '  const { title } = await res.json();\n' +
      "  const h = document.createElement('h1');\n" +
      "  h.textContent = title + ' ' + 'x'.repeat(200);\n" +
      '  document.body.append(h);\n' +
      '}\n' +
      'export function neverCalled() { return [1, 2, 3].map((n) => n * 2).join(","); }\n',
  );
  fs.writeFileSync(path.join(dir, 'data.0123456789abcdef.js'), 'export const data = 1;\n');
  // A debugger statement must not stop the diagnostic run (it enables the debugger).
  fs.writeFileSync(
    path.join(dir, 'app.js'),
    "import './data.0123456789abcdef.js';\ndebugger;\nconst { boot } = await import('./core.js');\nboot();\n",
  );
  fs.writeFileSync(
    path.join(dir, 'index.html'),
    '<!doctype html><div id="splash">loading</div><script type="module" src="/app.js"></script><body></body>',
  );
  const server = await startServer(dir);
  const browser = await launchBrowser({ profileDir: path.join(dir, 'chrome-profile') });
  t.after(async () => {
    await browser.close();
    await server.close();
    fs.rmSync(dir, { recursive: true, force: true });
  });
  const config = validateFieldConfig({
    url: `${server.origin}/`,
    variants: { base: { cookies: [{ name: 'variant', value: 'base' }], build: dir } },
    ignoreLcp: '#splash',
    gate: '/api/items',
  });
  const common = {
    config,
    context: undefined,
    saved: new Map(),
    redact: makeRedactor(),
    settleMs: 200,
    timeoutMs: 15_000,
    throttle: null,
    variant: config.variants.base,
  };
  const run = await timingRun(browser.cdp, common);
  assert.equal(run.invalid, null);
  assert.equal(typeof run.sample.lcp, 'number');
  assert.equal(run.entry, '/app.js');
  const path1 = criticalPath(run.requests, run.sample.lcp, { pattern: config.gate });
  assert.equal(path1.gate.url, `${server.origin}/api/items.json`);
  assert.deepEqual(
    path1.chain.map((s) => new URL(s.url).pathname),
    ['/core.js', '/app.js', '/'],
  );
  assert.equal(run.sample.lcpElement.text, undefined, 'page text is not kept');

  const diag = await diagnosticRun(browser.cdp, { ...common, cut: 'gate' });
  assert.equal(diag.invalid, null);
  const core = diag.scripts.find((s) => s.pathname === '/core.js');
  assert.ok(core?.atPaint, 'core.js executed before the gate request was sent');
  const index = indexBuild(dir);
  const body = diag.bodies.get(`${server.origin}/core.js`);
  assert.equal(matchScript(index, { file: 'core.js', body }).status, 'exact');
  const cov = chunkCoverage({ bytes: body.length, atCut: core.atPaint, atSettle: core.atSettle });
  assert.ok(cov.cutBytes > 0 && cov.cutBytes < cov.bytes, 'neverCalled() stays cold');
  const data = diag.scriptFetches.find((f) => f.pathname === '/data.0123456789abcdef.js');
  assert.ok(data?.prePaint, 'a chunk with a hex name keeps its raw path for matching');

  const unreached = await diagnosticRun(browser.cdp, {
    ...common,
    config: { ...config, gate: '/api/never' },
    cut: 'gate',
    timeoutMs: 1500,
  });
  assert.match(unreached.invalid, /was not sent within 1500ms/);
});
