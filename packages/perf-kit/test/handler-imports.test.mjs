// Imports that only code inside functions reads (lib/worker/handler-imports.mjs). Pinned here
// are the parts whose wrong answer is silent: a read at startup counted as a handler read (the
// lead tells the agent to defer code that startup needs), and an import source matched to the
// wrong graph module (the lead prices the wrong closure).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  asyncImportModules,
  importUses,
  loadParseAst,
  mainModuleIndex,
  matchImportTargets,
  parseFile,
  readOnlyInAsync,
  readOnlyLater,
  textModuleBytes,
} from '../lib/worker/handler-imports.mjs';
import { makeGraph } from './graph-fixtures.mjs';

const parseAst = await loadParseAst(path.dirname(fileURLToPath(import.meta.url)));

/** `{ name: [startup, fns] }` for every name the source imports. */
function uses(source, file = 'src/index.ts') {
  const program = parseFile(parseAst, file, source);
  assert.ok(program, 'the source parses');
  const out = {};
  for (const row of importUses(program)) {
    for (const n of row.names) out[n.name] = [n.startup, n.fns, n.reads];
  }
  return out;
}

test(
  'a read inside a handler is not a startup read; a module-scope call is',
  { skip: !parseAst },
  () => {
    const got = uses(`
    import { createApp } from './app';
    import { log } from './log';
    import { runCron } from './cron';
    log('boot');
    export default {
      fetch(req, env) { return createApp(env).fetch(req); },
      async scheduled(event, env) { await runCron(env); },
    };
  `);
    assert.deepEqual(got.createApp, [false, ['fetch'], 1]);
    assert.deepEqual(got.log, [true, [], 1]);
    assert.deepEqual(got.runCron, [false, ['scheduled'], 1]);
  },
);

test(
  'a function that module-scope code calls runs at startup, directly or through others',
  { skip: !parseAst },
  () => {
    const got = uses(`
    import { a } from 'a';
    import { b } from 'b';
    import { c } from 'c';
    function inner() { return a(); }
    function outer() { return inner(); }
    const later = () => c();
    outer();
    (() => { b(); })();
    export default { fetch: later };
  `);
    assert.equal(got.a[0], true);
    assert.equal(got.b[0], true);
    assert.deepEqual(got.c, [false, ['later'], 1]);
  },
);

test(
  'a static class field runs when the class is defined; an instance field per instance',
  { skip: !parseAst },
  () => {
    const got = uses(`
    import { s } from 's';
    import { i } from 'i';
    import { m } from 'm';
    class Store {
      static shared = s();
      cache = i();
      get() { return m(); }
    }
    export default { fetch() { return new Store().get(); } };
  `);
    assert.equal(got.s[0], true);
    assert.deepEqual(got.i, [false, ['cache'], 1]);
    assert.deepEqual(got.m, [false, ['get'], 1]);
  },
);

test('a module-scope value built from an import is startup code', { skip: !parseAst }, () => {
  const got = uses(`
    import { Hono } from 'hono';
    import { handler } from './routes';
    const app = new Hono();
    app.get('/', (c) => handler(c));
    export default app;
  `);
  assert.equal(got.Hono[0], true);
  assert.deepEqual(got.handler, [false, ['(function)'], 1]);
});

test('type imports and type positions read nothing', { skip: !parseAst }, () => {
  const program = parseFile(
    parseAst,
    'src/index.ts',
    `
    import type { Env } from './env';
    import { type Ctx, run } from './run';
    import { Schema } from './schema';
    import './polyfill';
    let ctx: Ctx | undefined;
    const schema: Schema = undefined as unknown as Schema;
    export default { fetch(req: Request, env: Env) { return run(env satisfies Env); } };
  `,
  );
  const rows = importUses(program);
  assert.deepEqual(
    rows.map((r) => [r.source, r.sideEffect, r.names.map((n) => n.name)]),
    [
      ['./run', false, ['run']],
      ['./schema', false, ['Schema']],
      ['./polyfill', true, []],
    ],
  );
  assert.equal(rows[1].names[0].reads, 0);
  assert.deepEqual(rows.map(readOnlyLater), [true, false, false]);
});

test(
  'a read inside an async function, at any depth, needs no caller change',
  { skip: !parseAst },
  () => {
    const program = parseFile(
      parseAst,
      'src/jobs.ts',
      `
    import { a } from 'a';
    import { b } from 'b';
    import { c } from 'c';
    import { d } from 'd';
    import { e } from 'e';
    export async function run() { return a(); }
    export function sync() { return b(); }
    export const later = async () => { [1].map(() => c()); };
    export class Job { async exec() { return d(); } }
    export async function both() { return e(); }
    export function also() { return e(); }
  `,
    );
    const rows = importUses(program);
    assert.deepEqual(
      rows.map((r) => [r.source, r.names[0].asyncOnly, readOnlyInAsync(r)]),
      [
        ['a', true, true],
        ['b', false, false],
        ['c', true, true],
        ['d', true, true],
        ['e', false, false],
      ],
    );
    // A read at startup is never async-only, even inside an async function that startup calls.
    const boot = importUses(
      parseFile(
        parseAst,
        'src/boot.ts',
        `import { f } from 'f'; async function init() { f(); } init();`,
      ),
    );
    assert.equal(boot[0].names[0].asyncOnly, false);
  },
);

test(
  'a JSX element name reads its binding; a lowercase tag reads none',
  { skip: !parseAst },
  () => {
    const got = uses(
      `
    import { Card } from './card';
    import * as ui from './ui';
    import { helper } from './helper';
    import { Later } from './later';
    const el = <Card title={helper()}><div /></Card>;
    export async function render() { return <ui.Panel><Later /></ui.Panel>; }
  `,
      'src/page.tsx',
    );
    assert.equal(got.Card[0], true);
    assert.equal(got.helper[0], true);
    assert.deepEqual(got.ui, [false, ['render'], 1]);
    assert.deepEqual(got.Later, [false, ['render'], 1]);
  },
);

test('an import can load later only when no name of it is read at startup', () => {
  const row = (...names) => ({ source: 'x', sideEffect: false, names });
  const n = (reads, startup) => ({ name: 'n', reads, startup, fns: [] });
  assert.equal(readOnlyLater(row(n(2, false), n(0, false))), true);
  assert.equal(readOnlyLater(row(n(2, false), n(1, true))), false);
  assert.equal(readOnlyLater(row(n(0, false))), false);
  assert.equal(readOnlyLater({ source: 'x', sideEffect: true, names: [] }), false);
});

test('the main module is the largest app module behind the virtual entry', () => {
  const graph = {
    entryModules: ['\\0virtual:cloudflare/worker-entry'],
    modules: [
      { id: '\\0virtual:cloudflare/worker-entry', imports: [[1, false]] },
      {
        id: '\\0virtual:cloudflare/export-types',
        imports: [
          [2, false],
          [3, false],
          [4, false],
          [5, true],
        ],
      },
      { id: 'src/env.ts', retainedBytes: 10, imports: [] },
      { id: 'src/index.ts', retainedBytes: 5000, imports: [] },
      { id: 'hono/dist/index.js', retainedBytes: 90000, imports: [] },
      { id: 'src/lazy.ts', retainedBytes: 99999, imports: [] },
    ],
  };
  const isApp = (id) => id.startsWith('src/');
  assert.equal(mainModuleIndex(graph, isApp), 3);
  assert.equal(mainModuleIndex({ ...graph, entryModules: ['src/env.ts'] }, isApp), 2);
  assert.equal(mainModuleIndex({ ...graph, entryModules: [] }, isApp), null);
});

test('import sources match graph modules by path, package, alias, and the last one left', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'perf-kit-handlers-'));
  try {
    const db = path.join(root, 'packages', 'db');
    fs.mkdirSync(path.join(db, 'src'), { recursive: true });
    fs.writeFileSync(path.join(db, 'package.json'), '{"name":"@acme/db"}');
    const app = path.join(root, 'apps', 'web');
    const ids = [
      'src/server/index.ts',
      'src/server/app.ts',
      'src/server/routes/index.ts',
      'hono/dist/index.js',
      '@hono/zod-openapi/dist/index.js',
      'pkg/dist/a.js',
      'pkg/dist/b.js',
      'src/lib/auth.ts',
      path.join(db, 'src', 'index.ts'),
      'src/server/lazy.ts',
      '\\0virtual:shim',
      'cloudflare:workers',
      'src/server/cron.ts',
    ];
    const graph = {
      modules: ids.map((id, i) => ({
        id,
        imports:
          i === 0 ? ids.slice(1).map((_, j) => [j + 1, ids[j + 1] === 'src/server/lazy.ts']) : [],
      })),
    };
    const sources = [
      './app.ts',
      './routes',
      'hono',
      '@hono/zod-openapi',
      'pkg/b',
      'pkg/a',
      '@/lib/auth',
      '@acme/db',
      'cloudflare:workers',
      '#cron',
    ];
    const found = matchImportTargets(graph, 0, sources, app);
    const byId = Object.fromEntries([...found].map(([s, i]) => [s, ids[i]]));
    assert.deepEqual(byId, {
      './app.ts': 'src/server/app.ts',
      './routes': 'src/server/routes/index.ts',
      hono: 'hono/dist/index.js',
      '@hono/zod-openapi': '@hono/zod-openapi/dist/index.js',
      'pkg/b': 'pkg/dist/b.js',
      'pkg/a': 'pkg/dist/a.js',
      '@/lib/auth': 'src/lib/auth.ts',
      '@acme/db': path.join(db, 'src', 'index.ts'),
      'cloudflare:workers': 'cloudflare:workers',
      // The last source left takes the last static edge: the dynamic edge and the virtual
      // module are no targets.
      '#cron': 'src/server/cron.ts',
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('only app modules with an import whose target retains enough are read', () => {
  // worker -> app -> big (40 modules) / small; worker -> pkg -> big2: a package is not read.
  const nodes = [
    {
      id: 'src/worker.ts',
      bytes: 1,
      imports: [
        [1, false],
        [4, false],
      ],
    },
    {
      id: 'src/app.ts',
      bytes: 1,
      imports: [
        [2, false],
        [3, false],
      ],
    },
    { id: 'src/big.ts', bytes: 20 * 1024, imports: [] },
    { id: 'src/small.ts', bytes: 100, imports: [] },
    { id: 'pkg/index.js', bytes: 1, imports: [[5, false]] },
    { id: 'pkg/big.js', bytes: 50 * 1024, imports: [] },
    { id: 'src/texts.ts', bytes: 10, imports: [] },
  ];
  // src/app.ts also imports a bundle of 60 text modules: few bytes, many modules.
  nodes[1].imports.push([6, false]);
  for (let i = 0; i < 60; i++) {
    nodes[6].imports.push([nodes.length, false]);
    nodes.push({
      id: `__CLOUDFLARE_MODULE__Text__/m${i}.sql__CLOUDFLARE_MODULE__`,
      bytes: 0,
      imports: [],
    });
  }
  const graph = makeGraph(nodes, ['src/worker.ts']);
  const isApp = (id) => id.startsWith('src/');
  const opts = { minBytes: 16 * 1024, minModules: 50 };
  const read = (o) => asyncImportModules(graph, isApp, { ...opts, ...o });
  assert.deepEqual(read({}), [0, 1]);
  assert.deepEqual(read({ skip: 0 }), [1]);
  // By modules alone: src/app.ts retains 64, the text bundle 61.
  assert.deepEqual(read({ minBytes: 64 * 1024 }), [0, 1]);
  assert.deepEqual(read({ minBytes: 64 * 1024, minModules: 62 }), [0]);
  assert.deepEqual(read({ minBytes: 64 * 1024, minModules: 100 }), []);
});

test('a text module counts its file on disk', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'perf-kit-text-'));
  try {
    const file = path.join(dir, 'migration.sql');
    fs.writeFileSync(file, 'x'.repeat(1000));
    const ids = [
      `__CLOUDFLARE_MODULE__Text__${file}__CLOUDFLARE_MODULE__`,
      `__CLOUDFLARE_MODULE__Data__${path.join(dir, 'gone.bin')}__CLOUDFLARE_MODULE__`,
      'src/app.ts',
    ];
    assert.deepEqual(textModuleBytes(ids.map((id) => ({ id, bytes: 0 }))), {
      count: 2,
      bytes: 1000,
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
