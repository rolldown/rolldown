import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { rolldown } from 'rolldown';
import type { OutputOptions } from 'rolldown';
import { expect, onTestFinished, test } from 'vitest';

async function bundle(
  files: Record<string, string>,
  output: OutputOptions = {},
  entries = ['main'],
  fallback: boolean | string = false,
) {
  const cwd = await mkdtemp(path.join(tmpdir(), 'rolldown-import-defer-'));
  onTestFinished(() => rm(cwd, { recursive: true, force: true }));
  for (const [name, source] of Object.entries(files)) {
    const file = path.join(cwd, name);
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, source);
  }
  const warnings: string[] = [];
  const build = await rolldown({
    cwd,
    input: Object.fromEntries(entries.map((name) => [name, `./${name}.mjs`])),
    experimental: { importDefer: true },
    onLog(level, log) {
      if (level === 'warn') warnings.push(log.message);
    },
  });
  try {
    await build.write({
      dir: 'out',
      entryFileNames: '[name].mjs',
      chunkFileNames: '[name]-[hash].mjs',
      ...output,
    });
  } finally {
    await build.close();
  }
  if (fallback) {
    expect(warnings.length).toBeGreaterThan(0);
    const reason = typeof fallback === 'string' ? fallback : 'would wait for them in sequence';
    for (const message of warnings) expect(message).toContain(reason);
  } else {
    expect(warnings).toEqual([]);
  }
  return (entry = 'main', args: string[] = []) =>
    execFileSync(process.execPath, [...args, `out/${entry}.mjs`], {
      cwd,
      encoding: 'utf8',
      timeout: 10_000,
    })
      .trim()
      .split(/\r?\n/);
}

const waitsForRelease =
  "console.log('a start'); await new Promise(r => globalThis.release = r); console.log('a end');";

test.each([false, true])('grouped eager order, transitive import %s', async (transitive) => {
  const run = await bundle(
    {
      'main.mjs': `import defer * as a from './a.mjs'; import './last.mjs'; ${transitive ? "import './b.mjs';" : ''} console.log('main', typeof a);`,
      'other.mjs': `import './last.mjs'; ${transitive ? '' : "import './a.mjs';"} console.log('other');`,
      'a.mjs': "console.log('a'); export const x = 1;",
      'last.mjs': "console.log('last');",
      'b.mjs': "import './a.mjs'; console.log('b');",
    },
    { codeSplitting: { groups: [{ name: 'deps', test: /[\\/](a|last)\.mjs$/ }] } },
    ['main', 'other'],
  );
  expect(run()).toEqual(transitive ? ['last', 'a', 'b', 'main object'] : ['last', 'main object']);
  expect(run('other')).toEqual(transitive ? ['last', 'other'] : ['last', 'a', 'other']);
});

test('entries evaluate members of the same manual group in their own order', async () => {
  const run = await bundle(
    {
      'main.mjs': "import defer * as a from './a.mjs'; import './last.mjs'; console.log(typeof a);",
      'first.mjs': "import './a.mjs'; import './last.mjs';",
      'other.mjs': "import './last.mjs'; import './a.mjs';",
      'a.mjs': "console.log('a'); export const x = 1;",
      'last.mjs': "console.log('last');",
    },
    { codeSplitting: { groups: [{ name: 'deps', test: /[\\/](a|last)\.mjs$/ }] } },
    ['main', 'other', 'first'],
  );
  expect(run()).toEqual(['last', 'object']);
  expect(run('other')).toEqual(['last', 'a']);
  expect(run('first')).toEqual(['a', 'last']);
});

test('an import between grouped dependencies keeps its position through re-exports', async () => {
  const run = await bundle(
    {
      'main.mjs':
        "import defer * as a from './a.mjs'; import './outside.mjs'; import './last.mjs'; console.log(typeof a);",
      'first.mjs': "import './barrel.mjs'; import './outside.mjs'; import './last.mjs';",
      'other.mjs': "import './last.mjs'; import './outside.mjs'; import './barrel.mjs';",
      'barrel.mjs': "export * from './a.mjs';",
      'a.mjs': "console.log('a'); export const x = 1;",
      'last.mjs': "console.log('last');",
      'outside.mjs': "console.log('outside');",
    },
    { codeSplitting: { groups: [{ name: 'deps', test: /[\\/](a|last)\.mjs$/ }] } },
    ['main', 'other', 'first'],
  );
  expect(run()).toEqual(['outside', 'last', 'object']);
  expect(run('other')).toEqual(['last', 'outside', 'a']);
  expect(run('first')).toEqual(['a', 'outside', 'last']);
});

test('a grouped sibling keeps independent dynamic imports on the native loader', async () => {
  const run = await bundle(
    {
      'main.mjs':
        "import defer * as a from './a.mjs'; import { load } from './last.mjs'; console.log(typeof a); await load();",
      'first.mjs': "import './a.mjs'; import './last.mjs';",
      'a.mjs': "console.log('a'); export const x = 1;",
      'last.mjs': "console.log('last'); export const load = () => import('./async.mjs');",
      'async.mjs': "import './wait.mjs'; import './release.mjs'; console.log('async');",
      'wait.mjs': waitsForRelease,
      'release.mjs': "console.log('release'); globalThis.release();",
    },
    {
      codeSplitting: {
        groups: [
          { name: 'deps', test: /[\\/](a|last)\.mjs$/ },
          { name: 'wait', test: /[\\/]wait\.mjs$/ },
          { name: 'release', test: /[\\/]release\.mjs$/ },
        ],
      },
    },
    ['main', 'first'],
  );
  expect(run()).toEqual(['last', 'object', 'a start', 'release', 'a end', 'async']);
  expect(run('first')).toEqual(['a', 'last']);
});

test('a fourth entry can load an independent member of a conflicting group', async () => {
  const run = await bundle(
    {
      'main.mjs': "import defer * as a from './a.mjs'; import './last.mjs'; console.log(typeof a);",
      'first.mjs': "import './a.mjs'; import './last.mjs';",
      'other.mjs': "import './last.mjs'; import './a.mjs';",
      'fourth.mjs': "import './third.mjs';",
      'a.mjs': "console.log('a'); export const x = 1;",
      'last.mjs': "console.log('last');",
      'third.mjs': "console.log('third');",
    },
    { codeSplitting: { groups: [{ name: 'deps', test: /[\\/](a|last|third)\.mjs$/ }] } },
    ['main', 'other', 'first', 'fourth'],
    'separate chunks',
  );
  expect(run()).toEqual(['last', 'object']);
  expect(run('other')).toEqual(['last', 'a']);
  expect(run('first')).toEqual(['a', 'last']);
  expect(run('fourth')).toEqual(['third']);
});

test('the loader starts independent siblings of an async grouped module', async () => {
  const run = await bundle(
    {
      'main.mjs': "import defer * as a from './a.mjs'; import './last.mjs'; console.log(typeof a);",
      'first.mjs': "import './a.mjs'; import './last.mjs';",
      'other.mjs': "import './last.mjs'; import './a.mjs';",
      'a.mjs': "console.log('a'); export const x = 1;",
      'last.mjs': "import './wait.mjs'; import './release.mjs'; console.log('last');",
      'wait.mjs': waitsForRelease,
      'release.mjs': "console.log('release'); globalThis.release();",
    },
    { codeSplitting: { groups: [{ name: 'deps', test: /[\\/](a|last)\.mjs$/ }] } },
    ['main', 'other', 'first'],
    'separate chunks',
  );
  expect(run()).toEqual(['a start', 'release', 'a end', 'last', 'object']);
  expect(run('first')).toEqual(['a', 'a start', 'release', 'a end', 'last']);
  expect(run('other')).toEqual(['a start', 'release', 'a', 'a end', 'last']);
});

test.each([false, true])(
  'an earlier chunk transitively waits for a group member, minify %s',
  async (minify) => {
    const run = await bundle(
      {
        'main.mjs': "import defer * as a from './a.mjs'; console.log('main', typeof a);",
        'first.mjs': "import './a.mjs'; import './last.mjs'; console.log('first');",
        'other.mjs': "import './relay.mjs'; import './a.mjs'; console.log('other');",
        'relay.mjs': "import './last.mjs'; console.log('relay');",
        'a.mjs':
          "globalThis.ready = true; globalThis.release?.(); console.log('a'); export const x = 1;",
        'last.mjs':
          "if (!globalThis.ready) await new Promise(r => globalThis.release = r); console.log('last');",
      },
      {
        minify,
        codeSplitting: {
          groups: [
            { name: 'deps', test: /[\\/](a|last)\.mjs$/ },
            { name: 'relay', test: /[\\/]relay\.mjs$/ },
          ],
        },
      },
      ['main', 'first', 'other'],
      'separate chunks',
    );
    expect(run()).toEqual(['main object']);
    expect(run('first')).toEqual(['a', 'last', 'first']);
    expect(run('other')).toEqual(['a', 'last', 'relay', 'other']);
  },
);

test.each([false, true])(
  'an eager sibling keeps its module scope, matching group %s',
  async (matches) => {
    const run = await bundle(
      {
        'main.mjs':
          "import defer * as a from './a.mjs'; import './last.mjs'; console.log('main', typeof a);",
        'first.mjs': "import './a.mjs'; import './last.mjs'; console.log('first');",
        'a.mjs': "console.log('a'); export const x = 1;",
        'last.mjs': "import './read.mjs'; console.log('last', typeof arguments);",
        'read.mjs': `
        try { (() => arguments)(); } catch (error) { console.log(error.name); }
        globalThis.arguments = 'global';
        console.log((() => arguments)(), (function () { return arguments.length })(1));
        delete globalThis.arguments;
      `,
      },
      {
        codeSplitting: {
          groups: [{ name: 'deps', test: matches ? /[\\/](a|last)\.mjs$/ : /[\\/]missing\.mjs$/ }],
        },
      },
      ['main', 'first'],
    );
    const sibling = ['ReferenceError', 'global 1', 'last undefined'];
    expect(run()).toEqual([...sibling, 'main object']);
    expect(run('first')).toEqual(['a', ...sibling, 'first']);
  },
);

test('eager re-exports keep both module scope and the request order of each entry', async () => {
  const run = await bundle(
    {
      'main.mjs':
        "import defer * as a from './a.mjs'; import { y } from './later.mjs'; console.log('main', typeof a, y);",
      'first.mjs':
        "import * as a from './middle.mjs'; import { y } from './later.mjs'; console.log('first', typeof a, y);",
      'other.mjs':
        "import { y } from './later.mjs'; import * as a from './middle.mjs'; console.log('other', typeof a, y);",
      'a.mjs': "console.log('a'); export const x = 1;",
      'last.mjs': "console.log('last', typeof arguments); export const y = 2;",
      'middle.mjs': "export * from './a.mjs';",
      'later.mjs': "export * from './last.mjs';",
    },
    { codeSplitting: { groups: [{ name: 'deps', test: /[\\/](a|last)\.mjs$/ }] } },
    ['main', 'first', 'other'],
  );
  expect(run()).toEqual(['last undefined', 'main object 2']);
  expect(run('first')).toEqual(['a', 'last undefined', 'first object 2']);
  expect(run('other')).toEqual(['last undefined', 'a', 'other object 2']);
});

test.each([false, true])(
  'private deferred requests prepare all async siblings together, minify %s',
  async (minify) => {
    const run = await bundle(
      {
        'main.mjs':
          "import defer * as ns from './target.mjs'; import defer * as extra from './extra.mjs'; console.log('main', ns.x, typeof extra);",
        'other.mjs': "console.log('other');",
        'target.mjs': "import './a.mjs'; import defer * as b from './b.mjs'; export const x = 1;",
        'a.mjs': waitsForRelease,
        'b.mjs': "console.log('b start'); await 0; console.log('b end');",
        'extra.mjs': "console.log('extra'); globalThis.release(); await 0;",
      },
      { minify },
      ['main', 'other'],
    );
    expect(run()).toEqual(['a start', 'b start', 'extra', 'b end', 'a end', 'main 1 object']);
    expect(run('other')).toEqual(['other']);
  },
);

test('a later deferred request can reject while an earlier request is pending', async () => {
  const run = await bundle({
    'main.mjs': "try { await import('./requests.mjs') } catch (e) { console.log(e.message) }",
    'requests.mjs':
      "import defer * as a from './a.mjs'; import defer * as b from './b.mjs'; console.log('unreachable');",
    'a.mjs': "console.log('waiting'); await new Promise(() => {});",
    'b.mjs': "console.log('rejecting'); await 0; throw new Error('failed');",
  });
  expect(run()).toEqual(['waiting', 'rejecting', 'failed']);
});

test('a cyclic reader sees the deferred namespace before its importer evaluates', async () => {
  const run = await bundle({
    'main.mjs':
      "import './reader.mjs'; import defer * as ns from './target.mjs'; export function read() { return typeof ns; } console.log('main');",
    'reader.mjs': "import { read } from './main.mjs'; console.log('reader', read());",
    'target.mjs': "console.log('target'); export const x = 1;",
  });
  expect(run()).toEqual(['reader object', 'main']);
});

test.each(['group size', 'global size', 'entries-aware merge'])(
  'chunk cycle from %s',
  async (mode) => {
    const merged = mode === 'entries-aware merge';
    const run = await bundle(
      {
        'main.mjs': `import './src/left/reporter.mjs'; ${merged ? "import './src/extra.mjs';" : ''} console.log('main');`,
        'fpage.mjs': "import './src/right/deferrer.mjs';",
        'tpage.mjs':
          "import { x } from './src/left/target.mjs'; import './src/extra.mjs'; console.log(x());",
        'src/extra.mjs': "console.log('extra');",
        'src/left/reporter.mjs':
          "import { value } from '../right/deferrer.mjs'; console.log('reporter', value);",
        // Keep these two modules above the merge threshold so only the smaller subgroups move.
        'src/left/target.mjs':
          'export function x() { return 1; }' + (merged ? '//' + 't'.repeat(180) : ''),
        'src/right/deferrer.mjs':
          "import defer * as ns from '../left/target.mjs'; console.log('deferrer', ns.x()); export const value = 'deferrer';" +
          (merged ? '//' + 'f'.repeat(180) : ''),
      },
      {
        codeSplitting: {
          maxSize: mode === 'global size' ? 150 : undefined,
          groups: [
            {
              name: 'shared',
              test: /[\\/]src[\\/]/,
              maxSize: mode === 'group size' ? 150 : undefined,
              entriesAware: merged,
              entriesAwareMergeThreshold: merged ? 130 : undefined,
            },
          ],
        },
      },
      merged ? ['main', 'fpage', 'tpage'] : ['main'],
    );
    expect(run()).toEqual([
      'deferrer 1',
      'reporter deferrer',
      ...(merged ? ['extra'] : []),
      'main',
    ]);
    if (merged) {
      expect(run('fpage')).toEqual(['deferrer 1']);
      expect(run('tpage')).toEqual(['extra', '1']);
    }
  },
);

test.each([false, true])(
  'async preparation with replaced Promise and Array.prototype.forEach, minify %s',
  async (minify) => {
    const run = await bundle(
      {
        'main.mjs': "import defer * as ns from './target.mjs'; console.log('main', ns.x);",
        'target.mjs':
          "import './a.mjs'; import './b.mjs'; console.log('target'); export const x = 1;",
        'a.mjs': "console.log('a start'); await 0; console.log('a end');",
        'b.mjs': "console.log('b start'); await 0; console.log('b end');",
        'setup.mjs': `
          globalThis.Promise = function () { throw Error('Global Promise called'); };
          Array.prototype.forEach = () => { throw Error('forEach called'); };
        `,
      },
      { preserveModules: true, minify },
    );
    expect(run('main', ['--import', './setup.mjs'])).toEqual([
      'a start',
      'b start',
      'a end',
      'b end',
      'target',
      'main 1',
    ]);
  },
);

test('an unrelated entry keeps concurrent preparation of siblings in one chunk', async () => {
  const run = await bundle(
    {
      'main.mjs': "import defer * as ns from './target.mjs'; console.log(ns.x);",
      'target.mjs': "import './a.mjs'; import defer * as b from './b.mjs'; export const x = 1;",
      'a.mjs': waitsForRelease,
      'b.mjs': "console.log('b start'); globalThis.release(); await 0; console.log('b end');",
      'unrelated.mjs': "console.log('unrelated');",
    },
    {},
    ['main', 'unrelated'],
  );
  expect(run()).toEqual(['a start', 'b start', 'a end', 'b end', '1']);
  expect(run('unrelated')).toEqual(['unrelated']);
});

test.each([
  ['separate', 'split'],
  ['separate', 'multi'],
  ['separate', 'preserve'],
  ['eager', 'split'],
  ['eager', 'preserve'],
  ['parent', 'split'],
  ['parent', 'preserve'],
  ['relay', 'split'],
  ['relay', 'preserve'],
])('independent imports start before the wait (%s, %s)', async (shape, layout) => {
  const parent = shape === 'parent' || shape === 'relay';
  const files = {
    'main.mjs': parent
      ? "import defer * as ns from './target.mjs'; console.log('main', ns.x);"
      : `${shape === 'eager' ? "import './a.mjs'" : "import defer * as a from './a.mjs'"}; import defer * as b from './b.mjs'; console.log('main', typeof b);`,
    'a.mjs': waitsForRelease,
    'b.mjs': "console.log('b start'); globalThis.release(); await 0; console.log('b end');",
    'use-a.mjs': "import './a.mjs';",
    'use-b.mjs': "import './b.mjs';",
    'target.mjs': `import './parent.mjs'; ${shape === 'relay' ? 'await 0;' : ''} export const x = 1;`,
    'parent.mjs': `import './a.mjs'; import './release.mjs'; ${shape === 'relay' ? '' : 'await 0;'} console.log('parent');`,
    'release.mjs': "console.log('release'); globalThis.release();",
  };
  const output: OutputOptions =
    layout === 'split'
      ? {
          codeSplitting: {
            groups: [
              { name: 'group-a', test: /[\\/]a\.mjs$/ },
              { name: 'group-b', test: /[\\/](b|release)\.mjs$/ },
            ],
          },
        }
      : { preserveModules: layout === 'preserve' };
  const run = await bundle(
    files,
    output,
    layout === 'multi' ? ['main', 'use-a', 'use-b'] : ['main'],
    true,
  );
  expect(run()).toEqual(
    parent
      ? ['a start', 'release', 'a end', 'parent', 'main 1']
      : ['a start', 'b start', 'a end', 'b end', 'main object'],
  );
});

test.each([true, false])(
  'an earlier request already starts the async dependency, deferred %s',
  async (deferred) => {
    const run = await bundle(
      {
        'main.mjs': `${deferred ? "import defer * as first from './target.mjs'" : "import './async.mjs'"}; import defer * as ns from './target.mjs'; console.log(typeof ns);`,
        'target.mjs': "import './async.mjs'; console.log('target'); export const x = 1;",
        'async.mjs': "await 0; console.log('async');",
      },
      { preserveModules: true },
    );
    expect(run()).toEqual(['async', 'object']);
  },
);

test.each(['async', 'sync', 'relay'])(
  'a later eager import of %s shares the prepared dependency',
  async (target) => {
    const first = target === 'async' ? 'async' : 'relay';
    const run = await bundle(
      {
        'main.mjs': "import defer * as ns from './outer.mjs'; console.log('main', typeof ns);",
        'outer.mjs': "import './parent.mjs'; console.log('outer');",
        'parent.mjs': `import defer * as ns from './${first}.mjs'; import './${target}.mjs'; await 0; console.log('parent');`,
        'relay.mjs': "import './async.mjs'; console.log('relay');",
        'async.mjs': "import './sync.mjs'; await 0; console.log('async');",
        'sync.mjs': "console.log('sync');",
      },
      { preserveModules: true },
    );
    expect(run()).toEqual([
      'sync',
      'async',
      ...(target === 'relay' ? ['relay'] : []),
      'parent',
      'main object',
    ]);
  },
);

test('a sync dependency reached only during preparation still needs to start', async () => {
  const run = await bundle(
    {
      'main.mjs': "import defer * as ns from './parent.mjs'; console.log('main', typeof ns);",
      'parent.mjs':
        "import defer * as ns from './relay.mjs'; import './release.mjs'; await 0; console.log('parent');",
      'relay.mjs': "import './async.mjs'; import './release.mjs';",
      'async.mjs': waitsForRelease,
      'release.mjs': "console.log('release'); globalThis.release();",
    },
    { preserveModules: true },
    ['main'],
    true,
  );
  expect(run()).toEqual(['a start', 'release', 'a end', 'parent', 'main object']);
});

test.each([false, true])(
  'repeated async prerequisite, independent dependency %s',
  async (release) => {
    const run = await bundle(
      {
        'main.mjs': "import defer * as ns from './outer.mjs'; console.log('main', typeof ns);",
        'outer.mjs': "import './parent.mjs'; console.log('outer');",
        'parent.mjs':
          "import defer * as ns from './relay.mjs'; import './nested.mjs'; await 0; console.log('parent');",
        'nested.mjs': `import defer * as ns from './relay.mjs'; ${release ? "import './release.mjs';" : ''} await 0; console.log('nested');`,
        'relay.mjs': "import './async.mjs'; console.log('relay');",
        'async.mjs': release ? waitsForRelease : "await 0; console.log('async');",
        'release.mjs': "console.log('release'); globalThis.release();",
      },
      { preserveModules: true },
      ['main'],
      release,
    );
    expect(run()).toEqual(
      release
        ? ['a start', 'release', 'a end', 'relay', 'nested', 'parent', 'outer', 'main object']
        : ['async', 'nested', 'parent', 'main object'],
    );
  },
);

test('a sibling that waits for just one of two async dependencies can release the other', async () => {
  const run = await bundle(
    {
      'main.mjs': "import defer * as ns from './parent.mjs'; console.log('main', typeof ns);",
      'parent.mjs':
        "import defer * as ns from './both.mjs'; import './release.mjs'; await 0; console.log('parent');",
      'both.mjs': "import './blocked.mjs'; import './ready.mjs';",
      'blocked.mjs': waitsForRelease,
      'ready.mjs': "await 0; console.log('ready');",
      'release.mjs': "import './ready.mjs'; console.log('release'); globalThis.release();",
    },
    { preserveModules: true },
    ['main'],
    true,
  );
  expect(run()).toEqual(['a start', 'ready', 'release', 'a end', 'parent', 'main object']);
});
