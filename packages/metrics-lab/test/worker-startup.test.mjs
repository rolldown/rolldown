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
  summarizeProfile,
  summarizeWorkerStartup,
} from '../lib/worker/measure.mjs';
import { evaluatingModules, whyModule } from '../lib/worker/attrib.mjs';
import { viteConfigShim } from '../lib/worker/config.mjs';
import { entryIdx, minCut } from '../lib/min-cut.mjs';
import { indexOf, makeGraph } from './graph-fixtures.mjs';

const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), 'metrics-lab-worker-'));
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
  assert.ok(shim.includes("mode: 'metrics'"));
  assert.ok(shim.includes('metricsDir'));
  assert.ok(shim.includes("name !== 'client'"));
  assert.ok(shim.includes('preserveModules: true'));
  assert.ok(shim.includes('dist-attrib/${name}'));
  assert.ok(shim.includes('METRICS_LAB_WORKER_ONLY'));
});
