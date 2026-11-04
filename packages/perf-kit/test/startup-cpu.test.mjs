// Measured startup CPU on the module graph (lib/worker/startup-cpu.mjs) and the paired
// keep rule (lib/worker/measure.mjs).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { deferAllInto, evalOverrides } from '../lib/module-graph.mjs';
import {
  closureCall,
  closureOnlyShrank,
  keepVerdict,
  needsMorePairs,
  pairedCall,
  pairedVerdict,
  signTail,
} from '../lib/worker/measure.mjs';
import {
  classModules,
  cpuByModule,
  cpuByOutputFile,
  cpuOf,
  isVirtual,
  lazyInStartupChunks,
  leaveOnlyTogether,
  libraryShare,
  startupGroups,
  wrapperPrice,
} from '../lib/worker/startup-cpu.mjs';
import { indexOf, makeGraph } from './graph-fixtures.mjs';

// virtual entry -> worker.ts; the request path and a cron job share seed.ts; a Workflow
// class holds an AI SDK; zod is a library every part imports.
const graph = makeGraph(
  [
    { id: '\\0virtual:entry', bytes: 0, imports: [[1, false]] },
    {
      id: 'src/worker.ts',
      bytes: 1,
      imports: [
        [2, false],
        [3, false],
        [4, false],
        [6, false],
      ],
    },
    {
      id: 'src/app.ts',
      bytes: 10,
      imports: [
        [5, false],
        [6, false],
      ],
    },
    {
      id: 'src/cron.ts',
      bytes: 10,
      imports: [
        [5, false],
        [7, false],
      ],
    },
    { id: 'src/flows/image.ts', bytes: 10, imports: [[8, false]] },
    { id: 'src/seed.ts', bytes: 100, imports: [] },
    { id: 'zod/index.js', bytes: 50, imports: [] },
    { id: 'src/cron-jobs.ts', bytes: 10, imports: [] },
    { id: 'ai/index.js', bytes: 200, imports: [] },
  ],
  ['\\0virtual:entry'],
);
const bySource = {
  '../src/app.ts': 1,
  '../src/cron.ts': 0.5,
  '../src/seed.ts': 6,
  '../src/flows/image.ts': 0.2,
  '../node_modules/.bun/ai@6/node_modules/ai/index.js': 9,
  '../node_modules/zod/index.js': 2,
  '../src/cron-jobs.ts': 1,
  '../src/gone.ts': 0.7,
};
const isApp = (id) => isVirtual(id) || id.startsWith('src/');

test('profile sources map onto graph ids, package stores included', () => {
  const cpu = cpuByModule(graph, bySource);
  assert.equal(cpu.byIdx.get(indexOf(graph, 'ai/index.js')), 9);
  assert.equal(cpu.byIdx.get(indexOf(graph, 'zod/index.js')), 2);
  assert.equal(cpu.byIdx.get(indexOf(graph, 'src/seed.ts')), 6);
  assert.equal(cpu.unmatchedMs, 0.7);
});

test('startup CPU by import: the rows are the dominator children under the app entry', () => {
  const groups = startupGroups(graph, cpuByModule(graph, bySource), { isApp });
  assert.deepEqual(groups.chain, ['\\0virtual:entry', 'src/worker.ts']);
  assert.equal(groups.anchor.ms, 19.7);
  assert.deepEqual(
    groups.rows.map((r) => [r.id, r.ms, r.modules, r.importers]),
    [
      ['src/flows/image.ts', 9.2, 2, 1],
      ['src/seed.ts', 6, 1, 2],
      ['zod/index.js', 2, 1, 2],
      ['src/cron.ts', 1.5, 2, 1],
      ['src/app.ts', 1, 1, 1],
    ],
  );
  assert.deepEqual(
    groups.rows[0].children.map((c) => [c.id, c.ms]),
    [['ai/index.js', 9]],
  );
});

test('a module holding most of the time stays a row; the descent never enters a library', () => {
  const hot = { ...bySource, '../src/seed.ts': 500 };
  const groups = startupGroups(graph, cpuByModule(graph, hot), { isApp });
  assert.equal(groups.anchor.id, 'src/worker.ts');
  assert.equal(groups.rows[0].id, 'src/seed.ts');
  const lib = makeGraph(
    [
      { id: '\\0virtual:entry', bytes: 0, imports: [[1, false]] },
      { id: 'src/worker.ts', bytes: 1, imports: [[2, false]] },
      { id: 'zod/index.js', bytes: 1, imports: [[3, false]] },
      { id: 'zod/core.js', bytes: 1, imports: [] },
    ],
    ['\\0virtual:entry'],
  );
  const g = startupGroups(lib, cpuByModule(lib, { '../node_modules/zod/core.js': 5 }), { isApp });
  assert.equal(g.anchor.id, 'src/worker.ts');
  assert.deepEqual(
    g.rows.map((r) => r.id),
    ['zod/index.js'],
  );
});

test('a module that two parts share moves only with both: the group prices what one cannot', () => {
  const cpu = cpuByModule(graph, bySource);
  const price = (ids) => {
    const removed = evalOverrides(
      graph,
      ids.flatMap((id) => deferAllInto(graph, indexOf(graph, id))),
    ).removed;
    return cpuOf(graph, cpu, removed);
  };
  assert.equal(price(['src/app.ts']), 1);
  assert.equal(price(['src/cron.ts']), 1.5);
  assert.equal(price(['src/app.ts', 'src/cron.ts']), 8.5);
});

test('class modules are found by their declaration, and wrappers price the closures together', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'perf-kit-classes-'));
  try {
    fs.mkdirSync(path.join(root, 'src/flows'), { recursive: true });
    fs.writeFileSync(
      path.join(root, 'src/flows/image.ts'),
      'export class ImageWorkflow extends WorkflowEntrypoint {}\n',
    );
    fs.writeFileSync(path.join(root, 'src/app.ts'), '// ImageWorkflowX is not a class here\n');
    const found = classModules(graph, ['ImageWorkflow', 'MissingRoom'], root);
    assert.deepEqual(
      found.map((c) => [c.name, c.id]),
      [['ImageWorkflow', 'src/flows/image.ts']],
    );
    const price = wrapperPrice(graph, cpuByModule(graph, bySource), found);
    assert.equal(price.ms, 9.2);
    assert.equal(price.removedCount, 2);
    assert.deepEqual(
      price.perClass.map((c) => [c.name, c.ms]),
      [['ImageWorkflow', 9.2]],
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('a module that a class and a handler import both reach leaves only when both move', () => {
  // entry -> worker; worker imports the app (read only in fetch) and exports a Workflow
  // class; the app and the class both import the shared core, which imports a library
  const g = makeGraph(
    [
      { id: '\\0virtual:entry', bytes: 0, imports: [[1, false]] },
      {
        id: 'src/worker.ts',
        bytes: 1,
        imports: [
          [2, false],
          [3, false],
        ],
      },
      { id: 'src/app.ts', bytes: 10, imports: [[4, false]] },
      { id: 'src/flows/job.ts', bytes: 20, imports: [[4, false]] },
      { id: 'src/core.ts', bytes: 300, imports: [[5, false]] },
      { id: 'lib/index.js', bytes: 400, imports: [] },
    ],
    ['\\0virtual:entry'],
  );
  const handler = [{ from: 1, to: 2, kind: 'defer' }];
  const wrapper = deferAllInto(g, indexOf(g, 'src/flows/job.ts'));
  assert.deepEqual(
    evalOverrides(g, handler).removed.map((m) => m.id),
    ['src/app.ts'],
  );
  assert.deepEqual(
    evalOverrides(g, wrapper).removed.map((m) => m.id),
    ['src/flows/job.ts'],
  );
  assert.deepEqual(
    leaveOnlyTogether(g, wrapper, handler).map((m) => m.id),
    ['lib/index.js', 'src/core.ts'],
  );
  assert.deepEqual(leaveOnlyTogether(g, wrapper, []), []);
});

test('wrappers: what each class holds alone, and what leaves only with every wrapper', () => {
  // entry -> worker -> two Workflow modules; both import the SDK, one also its own helper
  const g = makeGraph(
    [
      { id: '\\0virtual:entry', bytes: 0, imports: [[1, false]] },
      {
        id: 'src/worker.ts',
        bytes: 1,
        imports: [
          [2, false],
          [3, false],
        ],
      },
      {
        id: 'src/flows/image.ts',
        bytes: 10,
        imports: [
          [4, false],
          [5, false],
        ],
      },
      { id: 'src/flows/music.ts', bytes: 20, imports: [[5, false]] },
      { id: 'src/flows/image-helper.ts', bytes: 30, imports: [] },
      { id: 'sdk/index.js', bytes: 400, imports: [] },
    ],
    ['\\0virtual:entry'],
  );
  const cpu = cpuByModule(g, {
    '../src/flows/image.ts': 1,
    '../src/flows/image-helper.ts': 2,
    '../src/flows/music.ts': 0.5,
    '../node_modules/sdk/index.js': 8,
  });
  const classes = [
    { name: 'ImageWorkflow', index: indexOf(g, 'src/flows/image.ts'), id: 'src/flows/image.ts' },
    { name: 'MusicWorkflow', index: indexOf(g, 'src/flows/music.ts'), id: 'src/flows/music.ts' },
  ];
  const price = wrapperPrice(g, cpu, classes);
  assert.equal(price.removedCount, 4);
  assert.equal(price.removedBytes, 460);
  assert.equal(price.ms, 11.5);
  assert.deepEqual(
    price.perClass.map((c) => [c.name, c.ms, c.modules, c.bytes]),
    [
      ['ImageWorkflow', 3, 2, 40],
      ['MusicWorkflow', 0.5, 1, 20],
    ],
  );
  assert.deepEqual(price.shared, { count: 1, bytes: 400, ms: 8 });
  assert.equal(price.removed.length, 4);
});

test('paired keep rule: the floor, or every pair on one side', () => {
  const call = (pairs, deltaMs) =>
    pairedCall(pairedVerdict({ pairs, deltaMs, appStartupMs: 30 }), pairs.length);
  assert.equal(call([-9.6, -9.4, -9.7, -10.3, -9.8], -9.7), 'IMPROVEMENT beyond noise');
  // a small change under the floor: lower in every pair
  assert.equal(
    call([-1.7, -0.7, -1.2, -1.3, -1], -1.2),
    'IMPROVEMENT beyond noise - small: lower in all 5 pairs (under the 3ms floor)',
  );
  assert.equal(
    call([0.8, 1.2, 0.6, 1.1, 0.9], 0.9),
    'REGRESSION beyond noise - small: higher in all 5 pairs (under the 3ms floor)',
  );
  // pairs on both sides of zero
  assert.equal(
    call([-1.5, 3.9, 0.1, -1.6, -2], -1.5),
    'within noise (floor 3ms) - no effect proven',
  );
  // one-sided, but too few pairs or too small a median
  assert.equal(call([-1, -1, -1, -1], -1), 'within noise (floor 3ms) - no effect proven');
  assert.equal(
    call([-0.3, -0.2, -0.4, -0.1, -0.3], -0.3),
    'within noise (floor 3ms) - no effect proven',
  );
});

test('beyond the floor the pairs must agree: a few slow runs do not make a verdict', () => {
  const verdict = (pairs, deltaMs) => pairedVerdict({ pairs, deltaMs, appStartupMs: 29 });
  // the median is beyond the floor, but the pairs fall on both sides
  const mixed = [3.3, -1.4, 4.9, -0.5, 3.9];
  assert.equal(
    pairedCall(verdict(mixed, 3.3), 5),
    'within noise (floor 3ms) - the pairs disagree: the median is beyond the floor, higher in only 3 of 5 pairs',
  );
  assert.equal(needsMorePairs({ pairs: mixed, deltaMs: 3.3, appStartupMs: 29 }), true);
  // 4 of 5 lower still decides
  assert.equal(
    pairedCall(verdict([-4.4, 0.7, -0.2, -3.8, -4.5], -3.8), 5),
    'IMPROVEMENT beyond noise',
  );
  // 8 of 9 higher still decides
  assert.equal(
    pairedCall(verdict([-0.2, 6.7, 1.8, 6.9, 2.2, 7.8, 7, 5.7, 3.9], 5.7), 9),
    'REGRESSION beyond noise',
  );
  // 6 of 9 is not agreement, and 9 pairs get no more
  const six = [5, 5, 5, 5, 5, 5, -1, -1, -1];
  assert.equal(verdict(six, 5).kind, 'noise');
  assert.equal(needsMorePairs({ pairs: six, deltaMs: 5, appStartupMs: 29 }), false);
});

test('a rise that a fixed young generation does not repeat is heap sizing: the keep rule reads that series', () => {
  const rise = { pairs: [2.9, 2.6, 5.3, 5.9, 2.6], deltaMs: 2.9, appStartupMs: 29 };
  const flat = { pairs: [-0.7, -0.2, 1.6, 4.2, 1.2], deltaMs: 1.2, appStartupMs: 29 };
  const stays = { pairs: [4.1, 5.2, 3.9, 6, 4.4], deltaMs: 4.4, appStartupMs: 29 };
  assert.equal(pairedVerdict(rise).kind, 'regression');
  const keep = keepVerdict(rise, flat);
  assert.equal(keep.kind, 'noise');
  assert.equal(keep.heapSizing, true);
  assert.equal(keepVerdict(rise, stays).kind, 'regression');
  assert.equal(keepVerdict(rise, null).kind, 'regression');
  const better = { pairs: [-5, -6, -5.5, -4.8, -6.1], deltaMs: -5.5, appStartupMs: 29 };
  assert.equal(keepVerdict(better, stays).kind, 'improvement');
  // the KEEP rule then applies to the closure as for any noise verdict
  const closure = {
    left: [{ id: 'a', bytes: 251_700 }],
    entered: [],
    leftBytes: 251_700,
    enteredBytes: 0,
    stillBytes: 0,
    stillCount: 0,
    pinnedBytes: 3_990_000,
    pinnedModules: 2000,
  };
  assert.equal(closureCall(closure, null, keep).kind, 'keep');
  assert.equal(closureOnlyShrank(closure), true);
  assert.equal(closureOnlyShrank({ ...closure, entered: [{ id: 'b', bytes: 1 }] }), false);
  assert.equal(closureOnlyShrank({ ...closure, left: [] }), false);
  assert.equal(closureOnlyShrank(null), false);
});

test('sign test: a no-op puts 5 of 5 or 8 of 9 pairs on one side by chance at most 1 in 32', () => {
  assert.equal(signTail(5, 5), 1 / 32);
  assert.equal(signTail(4, 5), 6 / 32);
  assert.equal(signTail(8, 9), 10 / 512);
  assert.equal(signTail(7, 9), 46 / 512);
  const call = (pairs) => {
    const sorted = [...pairs].sort((a, b) => a - b);
    const deltaMs = sorted[(sorted.length - 1) / 2];
    return pairedCall(pairedVerdict({ pairs, deltaMs, appStartupMs: 30 }), pairs.length);
  };
  assert.equal(
    call([-1.2, -0.8, 0.4, -1.5, -1, -0.9, -1.3, -0.6, -1.1]),
    'IMPROVEMENT beyond noise - small: lower in 8 of 9 pairs (under the 3ms floor)',
  );
  assert.equal(
    call([-1.2, -0.8, 0.4, -1.5, -1, 0.9, -1.3, -0.6, -1.1]),
    'within noise (floor 3ms) - no effect proven',
  );
});

test('a series that leans one way without deciding gets more pairs', () => {
  const more = (pairs, deltaMs) => needsMorePairs({ pairs, deltaMs, appStartupMs: 30 });
  // A real series: a deferral that later measured -16ms
  assert.equal(more([-4.9, -0.3, -7.3, -1.6, 0.9], -1.6), true);
  assert.equal(more([-1.7, -0.7, -1.2, -1.3, -1], -1.2), false); // already decided
  assert.equal(more([-1.5, 3.9, 0.1, -1.6, -2], -1.5), false); // both sides
  assert.equal(more([-0.3, -0.2, 0.4, -0.1, -0.3], -0.2), false); // too small
  assert.equal(more([-1, -1, 1, -1, -1, -1, -1, -1, -1], -1), false); // already 9 pairs
});

test('within noise, a startup that lost enough bytes or modules is kept', () => {
  const noise = { kind: 'noise' };
  const KB = 1024;
  const mods = (n, bytes = 0) => Array.from({ length: n }, (_, i) => ({ id: `m${i}`, bytes }));
  const closure = (left, entered = [], o = {}) => ({
    left,
    entered,
    leftBytes: left.reduce((s, m) => s + m.bytes, 0),
    enteredBytes: entered.reduce((s, m) => s + m.bytes, 0),
    stillCount: 0,
    stillBytes: 0,
    pinnedBytes: 1000 * KB,
    pinnedModules: 1500,
    ...o,
  });
  // A timing verdict decides alone.
  assert.equal(closureCall(closure(mods(400, KB)), null, { kind: 'improvement' }), null);
  assert.equal(closureCall(closure(mods(400, KB)), null, { kind: 'regression' }), null);
  assert.deepEqual(closureCall(closure(mods(40, KB)), null, noise), {
    kind: 'keep',
    bytes: 40 * KB,
    modules: 40,
  });
  // A deferral of 261 SQL text modules: 15.9 KB in the graph.
  const sql = closure(mods(261, 62));
  assert.deepEqual(closureCall(sql, null, noise), { kind: 'keep', bytes: null, modules: 261 });
  // Modules a startup chunk still loads did not leave startup.
  assert.equal(
    closureCall(closure(mods(40, KB), [], { stillCount: 30, stillBytes: 30 * KB }), null, noise),
    null,
  );
  // The floor grows with the startup: 1% of the bytes, 10% of the modules.
  assert.equal(
    closureCall(closure(mods(40, KB), [], { pinnedBytes: 5000 * KB }), null, noise),
    null,
  );
  assert.equal(closureCall(closure(mods(60, 1), [], { pinnedModules: 1000 }), null, noise), null);
  // A chunk-group change moves only the loaded bytes.
  const loaded = (prev, curr) => ({ prev, curr, delta: curr - prev });
  assert.deepEqual(closureCall(closure([]), loaded(800 * KB, 700 * KB), noise), {
    kind: 'keep',
    bytes: 100 * KB,
    modules: 0,
  });
  assert.equal(closureCall(null, loaded(710.9 * KB, 683.7 * KB), noise), null);
  // Nothing moved: the deferral took nothing out of startup.
  assert.deepEqual(closureCall(closure([]), loaded(700 * KB, 700.5 * KB), noise), { kind: 'none' });
  assert.deepEqual(closureCall(closure([]), null, noise), { kind: 'none' });
  assert.equal(closureCall(closure(mods(3, KB)), null, noise), null);
  assert.equal(closureCall(null, null, noise), null);
});

test('attribution output files map onto graph ids; library self time is reported apart', () => {
  const evaluating = cpuByOutputFile(graph, {
    'src/app.js': 3,
    'src/flows/image.js': 4,
    'node_modules/.bun/zod@4/node_modules/zod/index.js': 2,
    'src/gone.js': 1,
  });
  assert.equal(evaluating.byIdx.get(indexOf(graph, 'src/app.ts')), 3);
  assert.equal(evaluating.byIdx.get(indexOf(graph, 'src/flows/image.ts')), 4);
  assert.equal(evaluating.byIdx.get(indexOf(graph, 'zod/index.js')), 2);
  assert.equal(evaluating.unmatchedMs, 1);
  const lib = libraryShare(graph, cpuByModule(graph, bySource), isApp);
  assert.equal(lib.ms, 11);
  assert.equal(lib.totalMs, 19.7);
  assert.deepEqual(lib.top, [
    ['ai', 9],
    ['zod', 2],
  ]);
});

test('a startup chunk that holds modules only dynamic imports reach is found', () => {
  const g = makeGraph(
    [
      { id: '\\0virtual:entry', bytes: 0, imports: [[1, false]] },
      {
        id: 'src/worker.ts',
        bytes: 1,
        imports: [
          [2, false],
          [3, true],
        ],
      },
      { id: 'hono/index.js', bytes: 10, imports: [] },
      { id: 'src/ai.ts', bytes: 1, imports: [[4, false]] },
      { id: '@ai-sdk/openai/index.js', bytes: 200, imports: [] },
    ],
    ['\\0virtual:entry'],
  );
  const lazy = lazyInStartupChunks(g, [
    { source: '../src/worker.ts', script: 'index.js', totalBytes: 1000 },
    { source: '../node_modules/hono/index.js', script: 'assets/vendor.js', totalBytes: 10000 },
    {
      source: '../node_modules/.bun/@ai-sdk+openai@3/node_modules/@ai-sdk/openai/index.js',
      script: 'assets/vendor.js',
      totalBytes: 200000,
    },
  ]);
  assert.equal(lazy.count, 1);
  assert.equal(lazy.bytes, 200000);
  assert.deepEqual(lazy.byScript, [['assets/vendor.js', 200000]]);
  assert.deepEqual(lazy.byPackage, [['@ai-sdk/openai', 200000]]);
});
