// Worker startup mode. Pinned here are the parts whose wrong answer is silent: a module
// rule that skips the entry (nothing runs, the floor reads as startup), a lab route the
// assets router answers with index.html (status 200, import never ran), coverage URLs
// that miss the output dir (a heavy module looks absent), and a top-level frame with
// line ticks that lands in "(unmapped)".

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import {
  COVERAGE_FLAGS,
  COVERAGE_MIN_WRANGLER,
  LAB_IMPORT_PATH,
  classifyModules,
  globToRegExp,
  labAssetsRouting,
  versionAtLeast,
  wrapperModule,
} from '../lib/worker/launch.mjs';
import {
  attributeCoverage,
  attributeProfile,
  localFile,
  packageOf,
  parseRequestSpec,
  summarizeProfile,
  summarizeWorkerStartup,
  WS_QUIET_MS,
  wsRequest,
} from '../lib/worker/measure.mjs';
import { evaluatingModules, shortModule, topLevelCalls, whyModule } from '../lib/worker/attrib.mjs';
import { attribOutDir, viteConfigShim } from '../lib/worker/config.mjs';
import { entryIdx, minCut } from '../lib/min-cut.mjs';
import { indexOf, makeGraph } from './graph-fixtures.mjs';

const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), 'perf-kit-worker-'));
const write = (dir, rel, text) => {
  const file = path.join(dir, rel);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
  return file;
};

// --- module rules -----------------------------------------------------------------

test('wrangler module-rule globs match the way wrangler matches them', () => {
  assert.ok(globToRegExp('**/*.js').test('index.js'));
  assert.ok(globToRegExp('**/*.js').test('assets/a-b.js'));
  assert.ok(globToRegExp('**/*.wasm').test('assets/esbuild.wasm'));
  assert.ok(!globToRegExp('**/*.js').test('index.jsx'));
  assert.ok(!globToRegExp('*.js').test('assets/a.js'));
  assert.ok(globToRegExp('assets/*.txt').test('assets/x.txt'));
});

test('every output file a rule classifies becomes a module; maps and configs never do', () => {
  const dir = tmp();
  write(dir, 'index.js', 'export default {}');
  write(dir, 'index.js.map', '{}');
  write(dir, 'wrangler.json', '{}');
  write(dir, 'wrangler.lab.json', '{}');
  write(dir, 'assets/a.js', '');
  write(dir, 'assets/w.wasm', '');
  write(dir, 'assets/t.txt', '');
  write(dir, 'assets/data.json', '{}');
  write(dir, '.vite/manifest.json', '{}');
  const { modules, skipped } = classifyModules(dir, [{ type: 'ESModule', include: ['**/*.js'] }]);
  const byRel = Object.fromEntries(modules.map((m) => [m.rel, m.type]));
  assert.deepEqual(byRel, {
    'index.js': 'ESModule',
    'assets/a.js': 'ESModule',
    'assets/w.wasm': 'CompiledWasm',
    'assets/t.txt': 'Text',
  });
  assert.deepEqual(skipped, ['assets/data.json']);
});

test('libraries a preserveModules build writes under node_modules/.pnpm are modules too', () => {
  const dir = tmp();
  write(dir, 'index.js', 'export default {}');
  write(dir, 'node_modules/.pnpm/zod@4.6.7/node_modules/zod/v4/classic/schemas.js', '');
  write(dir, '.dev.vars', 'A=1');
  write(dir, '.vite/manifest.js', '');
  const rels = classifyModules(dir).modules.map((m) => m.rel);
  assert.deepEqual(
    rels.sort((a, b) => a.localeCompare(b)),
    ['index.js', 'node_modules/.pnpm/zod@4.6.7/node_modules/zod/v4/classic/schemas.js'],
  );
});

// --- wrapper and routing ------------------------------------------------------------

test('the timing wrapper imports the target on the lab route and forwards everything else', () => {
  const m = wrapperModule('/out', 'index.js');
  assert.ok(m.path.endsWith('__lab_wrapper.js'));
  assert.ok(m.contents.includes(`'./index.js'`) || m.contents.includes('"./index.js"'));
  assert.ok(m.contents.includes(LAB_IMPORT_PATH));
  assert.ok(m.contents.includes('handler.fetch(request, env, ctx)'));
  const cov = wrapperModule('/out', 'index.js', { coverage: true });
  assert.ok(cov.contents.includes("from 'node:inspector/promises'"));
  assert.ok(cov.contents.includes('Profiler.startPreciseCoverage'));
  assert.ok(cov.contents.includes('Profiler.takePreciseCoverage'));
});

test('lab routes are served worker-first even behind SPA assets', () => {
  const assets = {
    directory: '/out/client',
    routerConfig: { static_routing: { user_worker: ['/api/*'] } },
    assetConfig: { not_found_handling: 'single-page-application' },
  };
  const routed = labAssetsRouting(assets);
  assert.deepEqual(routed.routerConfig.static_routing.user_worker, ['/__lab/*', '/api/*']);
  assert.equal(routed.routerConfig.has_user_worker, true);
  assert.deepEqual(assets.routerConfig.static_routing.user_worker, ['/api/*']); // input untouched
  assert.deepEqual(labAssetsRouting({ directory: '/x' }).routerConfig.static_routing.user_worker, [
    '/__lab/*',
  ]);
  assert.equal(labAssetsRouting(undefined), undefined);
});

test('a run_worker_first list gets the lab route too; a boolean stays as it is', () => {
  const list = { directory: '/x', run_worker_first: ['/agents/*'] };
  assert.deepEqual(labAssetsRouting(list).run_worker_first, ['/__lab/*', '/agents/*']);
  assert.deepEqual(list.run_worker_first, ['/agents/*']); // input untouched
  assert.equal(
    labAssetsRouting({ directory: '/x', run_worker_first: true }).run_worker_first,
    true,
  );
  assert.equal('run_worker_first' in labAssetsRouting({ directory: '/x' }), false);
});

test('the coverage run makes node:inspector importable on any compatibility date', () => {
  assert.deepEqual(COVERAGE_FLAGS, [
    'nodejs_compat',
    'enable_nodejs_inspector_local_dev',
    'enable_nodejs_inspector_module',
  ]);
});

test('a ws: request spec carries the path and the optional message', () => {
  assert.deepEqual(parseRequestSpec('/api/health'), {
    ws: false,
    path: '/api/health',
    message: null,
  });
  assert.deepEqual(parseRequestSpec('ws:/agents/chat/a'), {
    ws: true,
    path: '/agents/chat/a',
    message: null,
  });
  assert.deepEqual(parseRequestSpec('ws:/agents/chat/a|{"type":"rpc","args":["a|b"]}'), {
    ws: true,
    path: '/agents/chat/a',
    message: '{"type":"rpc","args":["a|b"]}',
  });
});

test('a ws: request is timed to the last server message, and a refused upgrade reports its status', async () => {
  const listeners = {};
  const socket = {
    accepted: false,
    sent: [],
    accept() {
      this.accepted = true;
    },
    send(text) {
      this.sent.push(text);
      setTimeout(() => listeners.message({ data: 'reply-after-load' }), 20);
    },
    close() {},
    addEventListener(type, fn) {
      listeners[type] = fn;
    },
  };
  const mf = {
    dispatchFetch: async () => ({ status: 101, webSocket: socket, text: async () => '' }),
  };
  const row = await wsRequest(mf, '/agents/chat/a', 'hello');
  assert.equal(socket.accepted, true);
  assert.deepEqual(socket.sent, ['hello']);
  assert.equal(row.status, 101);
  assert.equal(row.messages, 1);
  assert.equal(row.bytes, 'reply-after-load'.length);
  assert.ok(
    row.ms >= 15 && row.ms < WS_QUIET_MS,
    `timed to the reply, not the quiet wait: ${row.ms}`,
  );
  const refused = await wsRequest(
    { dispatchFetch: async () => ({ status: 404, webSocket: null, text: async () => 'no route' }) },
    '/nope',
    null,
  );
  assert.equal(refused.status, 404);
  assert.equal(refused.text, 'no route');
});

test('the coverage floor is a wrangler version, compared numerically', () => {
  assert.equal(COVERAGE_MIN_WRANGLER, '4.108.0');
  assert.ok(versionAtLeast('4.108.0', COVERAGE_MIN_WRANGLER));
  assert.ok(versionAtLeast('4.113.0', COVERAGE_MIN_WRANGLER));
  assert.ok(versionAtLeast('5.0.0-alpha', COVERAGE_MIN_WRANGLER));
  assert.ok(!versionAtLeast('4.90.0', COVERAGE_MIN_WRANGLER));
  assert.ok(!versionAtLeast('4.9.0', COVERAGE_MIN_WRANGLER));
  assert.ok(!versionAtLeast(null, COVERAGE_MIN_WRANGLER));
});

// --- timing summary -----------------------------------------------------------------

test('app startup is the main import minus the empty-import floor, never negative', () => {
  const s = summarizeWorkerStartup({
    floor: [2.0, 2.4, 2.2].map((ms) => ({ ms, bootMs: 40 })),
    main: [34.0, 35.0, 34.6].map((ms) => ({ ms, bootMs: 45 })),
  });
  assert.equal(s.runs, 3);
  assert.equal(s.metrics['worker.startup_ms'], 34.6);
  assert.equal(s.metrics['worker.boot_floor_ms'], 2.2);
  assert.equal(s.metrics['worker.app_startup_ms'], 32.4);
  assert.equal(s.metrics['worker.instance_boot_ms'], 45);
  assert.equal(s.guard.allRunsCompleted, true);
  assert.equal(s.guard.spreadPct, 2.9);
  const flat = summarizeWorkerStartup({
    floor: [3].map((ms) => ({ ms, bootMs: 1 })),
    main: [2].map((ms) => ({ ms, bootMs: 1 })),
  });
  assert.equal(flat.metrics['worker.app_startup_ms'], 0);
});

// --- a two-source chunk with a sourcemap, shared by the coverage and profile tests -------

// line 1 maps to source 0, line 2 to source 1 ("AAAA" = [0,0,0,0]; "ACAA" = [0,+1,0,0]).
function chunkFixture(dir, { sources = ['../src/a.ts', '../node_modules/lib/b.js'] } = {}) {
  write(dir, 'index.js', 'a();\nb();\n');
  write(
    dir,
    'index.js.map',
    JSON.stringify({ version: 3, sources, names: [], mappings: 'AAAA;ACAA' }),
  );
  write(dir, 'wrangler.json', JSON.stringify({ main: 'index.js' }));
  return dir;
}

test('coverage URLs resolve onto the output dir whether bare, file:// or from another directory', () => {
  const dir = chunkFixture(tmp());
  assert.equal(localFile('index.js', dir), path.join(dir, 'index.js'));
  assert.equal(localFile(`file://${path.join(dir, 'index.js')}`, dir), path.join(dir, 'index.js'));
  assert.equal(
    localFile('file:///somewhere/else/dist/worker/index.js', dir),
    path.join(dir, 'index.js'),
  );
  assert.equal(localFile('node-internal:events', dir), null);
  assert.equal(localFile('missing.js', dir), null);
});

test('coverage attribution: executed bytes per source, idle bytes, cold list and package view', () => {
  const dir = chunkFixture(tmp());
  const result = [
    {
      scriptId: '1',
      url: 'index.js',
      functions: [
        {
          functionName: '',
          ranges: [{ startOffset: 0, endOffset: 10, count: 1 }],
          isBlockCoverage: true,
        },
        {
          functionName: 'b',
          ranges: [{ startOffset: 5, endOffset: 10, count: 0 }],
          isBlockCoverage: true,
        },
      ],
    },
    { scriptId: '2', url: 'node-internal:events', functions: [] },
    { scriptId: '3', url: `file://${path.join(dir, '__lab_wrapper.js')}`, functions: [] },
  ];
  const cov = attributeCoverage(result, { outDir: dir });
  assert.deepEqual(cov.runtimeScripts, ['node-internal:events']);
  const bySource = Object.fromEntries(cov.modules.map((m) => [m.source, m]));
  assert.equal(bySource['../src/a.ts'].readyBytes, 5);
  assert.equal(bySource['../src/a.ts'].totalBytes, 5);
  assert.equal(bySource['../node_modules/lib/b.js'].readyBytes, 0);
  assert.equal(bySource['../node_modules/lib/b.js'].idleBytes, 5);
  assert.equal(cov.totals.readyBytes, 5);
  assert.equal(cov.totals.totalBytes, 10);
  assert.equal(cov.idle[0].source, '../node_modules/lib/b.js');
  assert.deepEqual(
    cov.packages.map((p) => [p.pkg, p.totalBytes, p.readyBytes]),
    [
      ['(app)', 5, 5],
      ['lib', 5, 0],
    ],
  );
  assert.equal(packageOf('../node_modules/@scope/pkg/dist/x.js'), '@scope/pkg');
  assert.equal(packageOf('../node_modules/a/node_modules/b/x.js'), 'b');
});

test('profile attribution splits a top-level frame by its line ticks and maps other chunks too', () => {
  const dir = chunkFixture(tmp());
  write(dir, 'assets/other.js', 'c();\n');
  write(
    dir,
    'assets/other.js.map',
    JSON.stringify({ version: 3, sources: ['../../src/c.ts'], names: [], mappings: 'AAAA' }),
  );
  const profile = {
    startTime: 0,
    endTime: 6000,
    nodes: [
      {
        id: 1,
        callFrame: { functionName: '(root)', url: '', lineNumber: -1, columnNumber: -1 },
        children: [2, 3, 4],
      },
      {
        id: 2,
        callFrame: {
          functionName: '',
          url: `file://${path.join(dir, 'index.js')}`,
          lineNumber: -1,
          columnNumber: -1,
        },
        positionTicks: [
          { line: 1, ticks: 3 },
          { line: 2, ticks: 1 },
        ],
      },
      {
        id: 3,
        callFrame: {
          functionName: 'c',
          url: `file://${path.join(dir, 'assets/other.js')}`,
          lineNumber: 0,
          columnNumber: 0,
        },
      },
      {
        id: 4,
        callFrame: {
          functionName: '(garbage collector)',
          url: '',
          lineNumber: -1,
          columnNumber: -1,
        },
      },
    ],
    samples: [2, 2, 2, 2, 3, 4],
    timeDeltas: [1000, 1000, 1000, 1000, 1000, 1000],
  };
  const prof = attributeProfile(profile, { outDir: dir, mainName: 'index.js' });
  const byBucket = Object.fromEntries(prof.rows.map((r) => [r.bucket, r.ms]));
  assert.equal(byBucket['../src/a.ts'], 3);
  assert.equal(byBucket['../node_modules/lib/b.js'], 1);
  assert.equal(byBucket['../../src/c.ts'], 1);
  assert.equal(byBucket['(engine: gc)'], 1);
  assert.equal(prof.ownership.appMs, 5);
  assert.equal(prof.ownership.engineMs, 1);
  const summary = summarizeProfile(profile);
  assert.deepEqual(summary, {
    windowMs: 6,
    sampledMs: 6,
    activeMs: 6,
    gcMs: 1,
    programMs: 0,
    idleMs: 0,
    samples: 6,
  });
});

test('a missing sourcemap yields the summary and no rows, not a crash', () => {
  const dir = tmp();
  write(dir, 'index.js', 'a();\n');
  const profile = {
    startTime: 0,
    endTime: 1000,
    nodes: [{ id: 1, callFrame: { functionName: '(root)', url: '' } }],
    samples: [1],
    timeDeltas: [1000],
  };
  const prof = attributeProfile(profile, { outDir: dir, mainName: 'index.js' });
  assert.equal(prof.rows, null);
  assert.equal(prof.missingSourcemap, true);
  assert.equal(prof.summary.samples, 1);
});

// --- attribution build: CPU by evaluating module -----------------------------------------

test('evaluating-module attribution charges a library frame to the root-most app module on the stack', () => {
  const dir = tmp();
  write(dir, 'worker/index.js', '');
  write(dir, 'node_modules/zod/core.js', '');
  write(dir, '_virtual/_rolldown/runtime.js', '');
  const url = (rel) => `file://${path.join(dir, rel)}`;
  const profile = {
    startTime: 0,
    endTime: 4000,
    nodes: [
      { id: 1, callFrame: { functionName: '(root)', url: '' }, children: [2, 5] },
      {
        id: 2,
        callFrame: {
          functionName: '',
          url: url('worker/index.js'),
          lineNumber: -1,
          columnNumber: -1,
        },
        children: [3],
      },
      {
        id: 3,
        callFrame: {
          functionName: 'init',
          url: url('node_modules/zod/core.js'),
          lineNumber: 10,
          columnNumber: 2,
        },
        children: [4],
      },
      {
        id: 4,
        callFrame: {
          functionName: '__esm',
          url: url('_virtual/_rolldown/runtime.js'),
          lineNumber: 1,
          columnNumber: 1,
        },
      },
      { id: 5, callFrame: { functionName: '(garbage collector)', url: '' } },
    ],
    samples: [3, 3, 4, 5],
    timeDeltas: [1000, 1000, 1000, 1000],
  };
  const out = evaluatingModules(profile, { outDir: dir });
  assert.deepEqual(
    out.rows.map((r) => [r.module, r.ms]),
    [
      ['worker/index.js', 3],
      ['(gc)', 1],
    ],
  );
  assert.deepEqual(
    out.pairs.map((p) => [p.pair, p.ms]),
    [['worker/index.js -> zod/core.js', 2]],
  );
  assert.equal(out.self.find((s) => s.file === 'node_modules/zod/core.js').ms, 2);
});

// --- the static side: roots and the why view ---------------------------------------------

function classGraph() {
  // entry -> ClassModule (a Durable Object class) -> heavy lib; entry -> small helper
  return makeGraph(
    [
      {
        id: 'worker/index.ts',
        bytes: 100,
        imports: [
          [1, false],
          [3, false],
        ],
      },
      { id: 'worker/agents/ThinkAgent.ts', bytes: 1000, imports: [[2, false]] },
      { id: 'just-bash/dist/bundle.js', bytes: 1_000_000, imports: [] },
      { id: 'worker/util.ts', bytes: 50, imports: [] },
    ],
    ['worker/index.ts'],
  );
}

test('with the class module as a root, the min cut lands below it instead of at the entry', () => {
  const graph = classGraph();
  const lib = indexOf(graph, 'just-bash/dist/bundle.js');
  const cls = indexOf(graph, 'worker/agents/ThinkAgent.ts');
  const withoutRoot = minCut(graph, lib);
  assert.equal(withoutRoot.flow, 1);
  const withRoot = minCut(graph, lib, [], [...entryIdx(graph), cls]);
  assert.deepEqual(withRoot.cutEdges, [{ from: cls, to: lib }]);
});

test('why: direct importers and the chain from the entry', () => {
  const graph = classGraph();
  const why = whyModule(graph, indexOf(graph, 'just-bash/dist/bundle.js'));
  assert.equal(why.module, 'just-bash/dist/bundle.js');
  assert.deepEqual(
    why.importers.map((i) => i.module),
    ['worker/agents/ThinkAgent.ts'],
  );
  assert.deepEqual(why.chain, ['worker/index.ts', 'worker/agents/ThinkAgent.ts']);
});

// --- the config shim ---------------------------------------------------------------------

test('the config shim puts the report on every environment and keeps the attribution build apart', () => {
  const shim = viteConfigShim('vite.config.ts');
  assert.ok(shim.includes("import base from './vite.config';"));
  assert.ok(shim.includes("mode: 'report'"));
  assert.ok(!shim.includes('reportDir'));
  assert.ok(shim.includes("name !== 'client'"));
  assert.ok(shim.includes('preserveModules: true'));
  assert.ok(shim.includes('dist-attrib/${name}'));
  assert.ok(shim.includes('ROLLDOWN_PERF_KIT_WORKER_ONLY'));
});

test('the worker-only rebuild puts back the assets field the Cloudflare plugin drops', () => {
  const shim = viteConfigShim('vite.config.ts');
  assert.ok(shim.includes("name: 'perf-kit:keep-assets'"));
  assert.ok(shim.includes("order: 'post'"));
  assert.ok(shim.includes('savedAssets.set(file, assets)'));
  assert.ok(shim.includes('config.assets = assets'));
});

test('worker-attrib finds the attribution build named after the environment', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'perf-kit-attrib-'));
  try {
    // TanStack Start: environment `ssr`, regular output `dist/server`.
    fs.mkdirSync(path.join(root, 'ssr'));
    fs.writeFileSync(path.join(root, 'ssr', 'wrangler.json'), '{}');
    assert.equal(attribOutDir(root, 'server'), path.join(root, 'ssr'));
    // A directory that matches the worker name wins.
    fs.mkdirSync(path.join(root, 'server'));
    fs.writeFileSync(path.join(root, 'server', 'wrangler.json'), '{}');
    assert.equal(attribOutDir(root, 'server'), path.join(root, 'server'));
    // Two candidates and no match: keep the worker name, so the caller's error names it.
    assert.equal(attribOutDir(root, 'worker'), path.join(root, 'worker'));
    // No attribution build at all.
    assert.equal(
      attribOutDir(path.join(root, 'missing'), 'server'),
      path.join(root, 'missing', 'server'),
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('a library call at module scope shows by name, charged to the module whose top level makes it', () => {
  const dir = tmp();
  write(dir, 'index.js', '');
  write(dir, 'node_modules/.pnpm/sdk@1.0.0/node_modules/sdk/dist/src.js', '');
  write(dir, 'node_modules/zod/core.js', '');
  const url = (rel) => `file://${path.join(dir, rel)}`;
  const frame = (functionName, rel, lineNumber) => ({
    functionName,
    url: rel ? url(rel) : '',
    lineNumber,
    columnNumber: 0,
  });
  const profile = {
    nodes: [
      { id: 1, callFrame: frame('(root)', null, -1), children: [2] },
      { id: 2, callFrame: frame('', 'index.js', -1), children: [3] },
      {
        id: 3,
        callFrame: frame(
          'preloadSchemas',
          'node_modules/.pnpm/sdk@1.0.0/node_modules/sdk/dist/src.js',
          41,
        ),
        children: [4],
      },
      { id: 4, callFrame: frame('build', 'node_modules/zod/core.js', 9) },
    ],
    // Three samples inside the warm-up (one in the call, two below it), one in index.js itself.
    samples: [3, 4, 4, 2],
    timeDeltas: [1000, 1000, 1000, 1000],
  };
  const fileOf = (u) => (u ? path.relative(dir, new URL(u).pathname) : null);
  const calls = topLevelCalls(profile, { fileOf });
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0], {
    module: 'index.js',
    fn: 'preloadSchemas',
    at: 'node_modules/.pnpm/sdk@1.0.0/node_modules/sdk/dist/src.js:42',
    source: null,
    ms: 3,
  });
  assert.equal(shortModule(calls[0].at), 'sdk/dist/src.js:42');
  assert.equal(
    shortModule('../../node_modules/.pnpm/zod@4.6.7/node_modules/zod/v4/core/core.js'),
    'zod/v4/core/core.js',
  );
  assert.equal(shortModule('src/server.ts'), 'src/server.ts');
});
