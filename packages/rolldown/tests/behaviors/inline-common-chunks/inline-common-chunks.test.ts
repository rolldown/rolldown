import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import type { ExperimentalInlineCommonChunksOptions, OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { describe, expect, test } from 'vitest';
import {
  assertRegistrationOrder,
  buildBoth,
  buildCase,
  carriers,
  chunkContaining,
  compareRoots,
  expectInlined,
  expectKeptAsFile,
  runRoot,
  runtimeChunk,
  type Built,
  type CaseOptions,
  type Modules,
} from '../../src/inline-common-chunks/harness';

// `experimentalInlineCommonChunks` prints a small common chunk into every file that reads it
// instead of writing it as its own file. Every case below builds twice, with the option off and
// on, runs every entry and dynamic entry as the root of a fresh Node process, and requires the
// same logs, exports, identities and errors. The registration order check parses the `on` output:
// in each file every `__share_require(id)` follows the `__share(id, ...)` of the same file.

// `String(globalThis.__x ?? ...)` keeps a value from being inlined as a constant, so the entries
// really read the shared binding.
const shared: Modules = {
  './shared.js': `
    globalThis.__log('S body');
    export let count = 0;
    export const marker = { tag: String(globalThis.__tag ?? 'shared') };
    export function bump() {
      count += 1;
      return this === undefined ? 'this:undefined' : 'this:bound';
    }
  `,
};

const twoEntries: Modules = {
  ...shared,
  './a.js': `
    import { bump, count, marker } from './shared.js';
    globalThis.__log('A', count, bump(), count, __id(marker));
    export const fromA = count;
  `,
  './b.js': `
    import { bump, count, marker } from './shared.js';
    globalThis.__log('B', count, bump?.(), count, __id(marker));
    export const fromB = marker.tag;
  `,
  './a-then-b.js': `
    await import('./a.js');
    await import('./b.js');
    globalThis.__log('a then b done');
  `,
  './b-then-a.js': `
    await import('./b.js');
    await import('./a.js');
    globalThis.__log('b then a done');
  `,
};

async function differential(name: string, options: CaseOptions) {
  const pair = await buildBoth(name, options);
  compareRoots(pair);
  if (!options.output?.minify) {
    assertRegistrationOrder(pair.on);
  }
  return pair;
}

describe('experimentalInlineCommonChunks', () => {
  test('two entries share one record, loaded in either order and interleaved', async () => {
    const pair = await differential('two-entries', {
      input: { a: './a.js', b: './b.js', 'a-then-b': './a-then-b.js', 'b-then-a': './b-then-a.js' },
      modules: twoEntries,
    });
    expectInlined(pair, 'shared.js');
    const a = chunkContaining(pair.on, 'a.js')!;
    expect(a.imports).toEqual([runtimeChunk(pair.on)!.fileName]);
    expect(a.moduleIds).toEqual(['./a.js', './shared.js']);
    expect(Object.keys(a.modules)).toEqual(expect.arrayContaining(['./shared.js', './a.js']));
    expect(a.modules['./shared.js'].code).toContain('S body');
  });

  test('a record reading another record: readers carry the closure, one runtime import each', async () => {
    const pair = await differential('record-reads-record', {
      input: { a: './a.js', b: './b.js', c: './c.js' },
      modules: {
        './s3.js': `
          globalThis.__log('S3');
          export const base = { n: Number(globalThis.__n ?? 3) };
          export function tag(strings, ...values) {
            return (this === undefined ? 'U' : 'B') + strings.join('|') + values.join(',');
          }
        `,
        './s1.js': `
          import { base, tag } from './s3.js';
          globalThis.__log('S1');
          export const one = base.n + 1;
          export const tagged = tag\`t\${one}\`;
          export class C { constructor() { this.k = 'c'; } }
        `,
        './a.js': `
          import { one, tagged, C } from './s1.js';
          import { base } from './s3.js';
          globalThis.__log('A', one, tagged, new C().k, base.n, __id(base));
        `,
        './b.js': `
          import * as ns from './s1.js';
          import { tag, base } from './s3.js';
          globalThis.__log('B', ns.one, tag\`b\${ns.one}\`, ns.tagged, new ns.C().k, __id(base));
        `,
        './c.js': `
          import { base, tag } from './s3.js';
          globalThis.__log('C', base.n, tag\`c\`, __id(base));
        `,
      },
    });
    expectInlined(pair, 's1.js');
    expectInlined(pair, 's3.js');
    const b = chunkContaining(pair.on, 'b.js')!;
    const runtimeFile = runtimeChunk(pair.on)!.fileName;
    expect(b.code.split(`from "./${runtimeFile}"`)).toHaveLength(2);
    const specifiers = b.code.match(/import \{([^}]*)\} from "\.\/rolldown-runtime[^"]*"/)![1];
    const locals = specifiers.split(',').map((s) => s.trim().split(/\s+/).at(-1));
    expect(new Set(locals).size).toBe(locals.length);
  });

  test('a module cycle inside a record, entered from each root', async () => {
    await differential('cycle-in-record', {
      input: { a: './a.js', b: './b.js' },
      modules: {
        './x.js': `
          import { y, yv } from './y.js';
          globalThis.__log('X body', yv);
          export function x() { return 'x(' + y() + ')'; }
          export const xv = String(globalThis.__xv ?? 'xv');
        `,
        './y.js': `
          import { x, xv } from './x.js';
          globalThis.__log('Y body', x(), xv);
          export function y() { return 'y'; }
          export const yv = String(globalThis.__yv ?? 'yv');
        `,
        './a.js': `import { x } from './x.js'; globalThis.__log('A', x());`,
        './b.js': `import { y, yv } from './y.js'; globalThis.__log('B', y(), yv);`,
      },
    });
  });

  test('CommonJS members: one module.exports, default and named interop, throw then require again', async () => {
    const pair = await differential('cjs-members', {
      input: { a: './a.js', b: './b.js', t1: './t1.js', t2: './t2.js', main: './main.js' },
      modules: {
        './cjs.js': `
          let n = 0;
          module.exports = {
            bump() { n += 1; return n; },
            self() { return module.exports; },
          };
          globalThis.__log('CJS body');
        `,
        './cjs-throw.js': `
          if (!globalThis.__thrown) {
            globalThis.__thrown = true;
            throw new Error('first load fails');
          }
          exports.ok = true;
        `,
        './a.js': `
          import cjs, { bump } from './cjs.js';
          globalThis.__log('A', bump(), cjs.self() === cjs, __id(cjs));
        `,
        './b.js': `
          import * as ns from './cjs.js';
          globalThis.__log('B', ns.default.bump(), ns.bump(), __id(ns.default));
        `,
        './t1.js': `import { ok } from './cjs-throw.js'; globalThis.__log('T1', ok);`,
        './t2.js': `import * as m from './cjs-throw.js'; globalThis.__log('T2', typeof m.default, m.ok);`,
        './main.js': `
          try {
            await import('./t1.js');
          } catch (e) {
            globalThis.__log('t1 failed', e.message);
          }
          await import('./t2.js');
          await import('./a.js');
          await import('./b.js');
        `,
      },
    });
    expectInlined(pair, 'cjs.js');
  });

  test('dynamic import consumers, including concurrent loads, share the record', async () => {
    const pair = await differential('dynamic-consumers', {
      input: { main: './main.js', s: './static.js' },
      modules: {
        ...shared,
        './lazy1.js': `
          import { bump, marker } from './shared.js';
          globalThis.__log('L1', bump(), __id(marker));
          export const v = 1;
        `,
        './lazy2.js': `
          import { bump, marker } from './shared.js';
          globalThis.__log('L2', bump(), __id(marker));
          export const v = 2;
        `,
        './static.js': `
          import { count, marker } from './shared.js';
          globalThis.__log('static', count, __id(marker));
        `,
        './main.js': `
          const [l1, l2] = await Promise.all([import('./lazy1.js'), import('./lazy2.js')]);
          globalThis.__log('main', l1.v, l2.v);
          await import('./static.js');
        `,
      },
    });
    expectInlined(pair, 'shared.js');
  });

  test('every call form keeps `this === undefined` for record functions', async () => {
    await differential('call-forms', {
      input: { a: './a.js', b: './b.js' },
      modules: {
        './shared.js': `
          export function f() { return this === undefined; }
          export function tag() { return this === undefined; }
          export class K { constructor() { this.ok = true; } }
          export const obj = { m() { return this === obj; } };
          export const holder = { f };
        `,
        './a.js': `
          import * as ns from './shared.js';
          import { f, tag, K, obj, holder } from './shared.js';
          globalThis.__log(
            f(), f?.(), ns.f(), ns.f?.(), tag\`x\`, ns.tag\`x\`, new K().ok, new ns.K().ok,
            obj.m(), ns.obj.m(), holder.f(), (0, f)(), [f][0](),
          );
        `,
        './b.js': `import { f } from './shared.js'; globalThis.__log('B', f());`,
      },
    });
  });

  test('minified output behaves the same', async () => {
    const pair = await differential('minify', {
      input: { a: './a.js', b: './b.js', 'a-then-b': './a-then-b.js' },
      modules: twoEntries,
      output: { minify: true },
    });
    expectInlined(pair, 'shared.js');
  });

  test('copied module code maps back to the original module', async () => {
    const on = await buildCase('sourcemap', 'on', {
      input: { a: './a.js', b: './b.js' },
      modules: twoEntries,
      output: { sourcemap: true },
    });
    const a = chunkContaining(on, 'a.js')!;
    expect(a.map).toBeTruthy();
    const tracer = new TraceMap(a.map!.toString());
    const lines = a.code.split('\n');
    const line = lines.findIndex((text) => text.includes("'S body'") || text.includes('"S body"'));
    expect(line).toBeGreaterThanOrEqual(0);
    const column = lines[line].indexOf('globalThis');
    const original = originalPositionFor(tracer, { line: line + 1, column });
    expect(original.source?.endsWith('shared.js')).toBe(true);
    expect(original.line).toBe(2);
  });

  test('a carrier hash changes with the record it prints', async () => {
    const hashed = (modules: Modules, name: string) =>
      buildCase(name, 'on', {
        input: { a: './a.js', b: './b.js' },
        modules,
        output: { entryFileNames: '[name]-[hash].js', chunkFileNames: '[name]-[hash].js' },
      });
    const before = await hashed(twoEntries, 'hash-before');
    const after = await hashed(
      { ...twoEntries, './shared.js': twoEntries['./shared.js'].replace("'S body'", "'S body!'") },
      'hash-after',
    );
    const names = (built: Built) => built.chunks.map((chunk) => chunk.fileName).sort();
    expect(names(before).filter((n) => n.startsWith('a-'))).not.toEqual(
      names(after).filter((n) => n.startsWith('a-')),
    );
    expect(names(before).filter((n) => n.startsWith('rolldown-runtime-'))).toEqual(
      names(after).filter((n) => n.startsWith('rolldown-runtime-')),
    );
  });

  test('`maxSize: 0` is byte-identical to leaving the option out', async () => {
    const generate = async (codeSplitting: object | undefined) => {
      const bundle = await rolldown({
        input: { a: './a.js', b: './b.js' },
        preserveEntrySignatures: false,
        plugins: [
          (await import('../../src/inline-common-chunks/harness')).virtualPlugin(twoEntries),
        ],
      });
      const { output } = await bundle.generate({
        format: 'esm',
        strictExecutionOrder: true,
        ...(codeSplitting ? { codeSplitting } : {}),
      });
      await bundle.close();
      return Object.fromEntries(
        output
          .filter((item): item is OutputChunk => item.type === 'chunk')
          .map((chunk) => [chunk.fileName, chunk.code]),
      );
    };
    const zero = await generate({ experimentalInlineCommonChunks: { maxSize: 0 } });
    const absent = await generate(undefined);
    expect(Object.keys(zero).length).toBeGreaterThan(2);
    expect(zero).toEqual(absent);
  });

  test('a record imports a file; carriers keep their own bindings and globals', async () => {
    const pair = await differential('record-imports-file', {
      input: { a: './a.js', b: './b.js', c: './c.js' },
      modules: {
        './c.js': `
          export const helper = String(globalThis.__h ?? 'h');
          globalThis.__log('C body');
        `,
        './s1.js': `
          import { helper } from './c.js';
          export const s1 = helper + '!';
          export const g = typeof globalThing;
          export const e = typeof exports;
        `,
        './a.js': `
          import { s1, g, e } from './s1.js';
          let helper = 'local';
          helper += '';
          var globalThing = 1;
          var exports = 2;
          globalThis.__log('A', helper, s1, g, e, globalThing + exports);
        `,
        './b.js': `import { s1, g } from './s1.js'; globalThis.__log('B', s1, g);`,
      },
    });
    expectInlined(pair, 's1.js');
    const a = chunkContaining(pair.on, 'a.js')!;
    // The record's import of `helper` is printed into `a.js`, so `a.js`'s own `helper` moved aside.
    expect(a.code).toMatch(/import \{[^}]*\bhelper\b[^}]*\} from "\.\/c/);
    expect(a.code).toContain('helper$1');
  });

  test('plugins see one renderChunk per file and truthful modules in generateBundle', async () => {
    const renderCounts: Record<string, number> = {};
    let bundleKeys: string[] = [];
    let carrierInfo: { moduleIds: string[]; modules: string[] } | undefined;
    const spy: Plugin = {
      name: 'spy',
      renderChunk(_code, chunk) {
        renderCounts[chunk.fileName] = (renderCounts[chunk.fileName] ?? 0) + 1;
        return null;
      },
      generateBundle(_options, bundle) {
        bundleKeys = Object.keys(bundle).sort();
        const a = bundle['a.js'];
        if (a?.type === 'chunk') {
          carrierInfo = { moduleIds: a.moduleIds, modules: Object.keys(a.modules) };
        }
      },
    };
    const on = await buildCase('plugin-hooks', 'on', {
      input: { a: './a.js', b: './b.js' },
      modules: twoEntries,
      plugins: [spy],
    });
    expect(
      carriers(on)
        .map((chunk) => chunk.fileName)
        .sort(),
    ).toEqual(['a.js', 'b.js']);
    expect(bundleKeys).toEqual(['a.js', 'b.js', 'rolldown-runtime.js']);
    expect(renderCounts).toEqual({ 'a.js': 1, 'b.js': 1, 'rolldown-runtime.js': 1 });
    expect(carrierInfo?.moduleIds).toContain('./shared.js');
    expect(carrierInfo?.modules).toContain('./shared.js');
  });

  test('`exclude` accepts a string, a RegExp, a function and an array of them', async () => {
    const base: CaseOptions = { input: { a: './a.js', b: './b.js' }, modules: twoEntries };
    const matchers: Array<[string, ExperimentalInlineCommonChunksOptions['exclude']]> = [
      ['string', 'shared\\.js$'],
      ['regexp', /shared\.js$/],
      ['function', (id: string) => id.endsWith('shared.js')],
      ['array', [/nothing/, (id: string) => id.endsWith('shared.js')]],
    ];
    for (const [label, exclude] of matchers) {
      const pair = await buildBoth(`exclude-${label}`, {
        ...base,
        inline: { maxSize: 1 << 20, exclude },
      });
      compareRoots(pair);
      expectKeptAsFile(pair, 'shared.js');
    }
    // The function is asked once per included module, before anything is selected, so a matcher
    // that rejects nothing still sees every module.
    const seen: string[] = [];
    const pair = await buildBoth('exclude-function-seen', {
      ...base,
      inline: {
        maxSize: 1 << 20,
        exclude: (id: string) => {
          seen.push(id);
          return false;
        },
      },
    });
    expectInlined(pair, 'shared.js');
    expect(seen.slice().sort()).toEqual(['./a.js', './b.js', './shared.js']);
  });
});

describe('kept as a file', () => {
  const ab = (sharedSource: string, extra: Modules = {}): Modules => ({
    './shared.js': sharedSource,
    './a.js': `import { s } from './shared.js'; globalThis.__log('A', s);`,
    './b.js': `import { s } from './shared.js'; globalThis.__log('B', s);`,
    ...extra,
  });
  const cases: Array<[string, CaseOptions, string]> = [
    [
      'a member with top-level await',
      {
        input: { a: './a.js', b: './b.js' },
        modules: ab(`await Promise.resolve(); export const s = String(globalThis.__s ?? 'S');`),
      },
      'shared.js',
    ],
    [
      'a member using import.meta',
      {
        input: { a: './a.js', b: './b.js' },
        modules: ab(`export const s = typeof import.meta.url;`),
      },
      'shared.js',
    ],
    [
      'a member with a retained dynamic import',
      {
        input: { a: './a.js', b: './b.js' },
        modules: ab(
          `export const s = String(globalThis.__s ?? 'S'); export const load = () => import('./lazy.js');`,
          {
            './lazy.js': `export const l = 1;`,
            './a.js': `import { s, load } from './shared.js'; const m = await load(); globalThis.__log('A', s, m.l);`,
          },
        ),
      },
      'shared.js',
    ],
    [
      'a member importing an external module',
      {
        input: { a: './a.js', b: './b.js' },
        modules: ab(`import { join } from 'node:path'; export const s = typeof join;`),
        inputOptions: { external: ['node:path'] },
      },
      'shared.js',
    ],
    [
      'a member using direct eval',
      { input: { a: './a.js', b: './b.js' }, modules: ab(`export const s = eval("'S'");`) },
      'shared.js',
    ],
    [
      'a reader using direct eval',
      {
        input: { a: './a.js', b: './b.js' },
        modules: ab(`export const s = String(globalThis.__s ?? 'S');`, {
          './a.js': `import { s } from './shared.js'; globalThis.__log('A', s, eval('typeof s'));`,
        }),
      },
      'shared.js',
    ],
    [
      'a declaration file',
      {
        input: { a: './a.js', b: './b.js' },
        modules: {
          './types.d.ts': `export const s = String(globalThis.__s ?? 'S');`,
          './a.js': `import { s } from './types.d.ts'; globalThis.__log('A', s);`,
          './b.js': `import { s } from './types.d.ts'; globalThis.__log('B', s);`,
        },
      },
      'types.d.ts',
    ],
    [
      'a chunk not smaller than maxSize',
      {
        input: { a: './a.js', b: './b.js' },
        modules: ab(`export const s = String(globalThis.__s ?? 'S');`),
        inline: { maxSize: 8 },
      },
      'shared.js',
    ],
    [
      'a manual group',
      {
        input: { a: './a.js', b: './b.js' },
        modules: ab(`export const s = String(globalThis.__s ?? 'S');`),
        output: { codeSplitting: { groups: [{ name: 'vendor', test: /shared\.js$/ }] } },
      },
      'shared.js',
    ],
    [
      'a chunk whose symbol another chunk re-exports',
      {
        input: { a: './a.js', b: './b.js' },
        modules: ab(`export const s = String(globalThis.__s ?? 'S');`, {
          './lazy.js': `export { s } from './shared.js';`,
          './a.js': `import { s } from './shared.js'; const m = await import('./lazy.js'); globalThis.__log('A', s, m.s);`,
        }),
      },
      'shared.js',
    ],
    [
      'a chunk hosting an entry module',
      {
        input: { b: './b.js', e: './e.js' },
        modules: {
          './shared.js': `globalThis.__log('S body'); export const s = String(globalThis.__s ?? 'S');`,
          './b.js': `import { s } from './shared.js'; globalThis.__log('B body', s);`,
          './e.js': `globalThis.__log('E body');`,
        },
        output: { codeSplitting: { groups: [{ name: 'e-group', test: /(?:e|shared)\.js$/ }] } },
      },
      'shared.js',
    ],
    [
      'a chunk in a static import cycle with a file',
      {
        input: { a: './a.js', b: './b.js' },
        modules: {
          './g.js': `import { r } from './r.js'; export const g = String(globalThis.__g ?? 'g'); export function readR() { return r; }`,
          './r.js': `import { g } from './g.js'; export const r = 'r' + g;`,
          './a.js': `import { readR } from './g.js'; import { r } from './r.js'; globalThis.__log('A', readR(), r);`,
          './b.js': `import { r } from './r.js'; globalThis.__log('B', r);`,
        },
        output: {
          codeSplitting: {
            groups: [{ name: 'grp', test: /g\.js$/, includeDependenciesRecursively: false }],
          },
        },
      },
      'r.js',
    ],
  ];

  test.each(cases)('%s stays a file', async (label, options, moduleSuffix) => {
    const pair = await buildBoth(`kept-${label.replace(/[^a-z0-9]+/gi, '-')}`, options);
    compareRoots(pair);
    expectKeptAsFile(pair, moduleSuffix);
  });
});

describe('preconditions', () => {
  const inline = { maxSize: 4096 };
  const modules = twoEntries;
  async function attempt(input: Record<string, unknown>, output: Record<string, unknown>) {
    const bundle = await rolldown({
      input: { a: './a.js', b: './b.js' },
      plugins: [(await import('../../src/inline-common-chunks/harness')).virtualPlugin(modules)],
      preserveEntrySignatures: false,
      ...input,
    });
    try {
      await bundle.generate({
        format: 'esm',
        strictExecutionOrder: true,
        codeSplitting: { experimentalInlineCommonChunks: inline },
        ...output,
      });
    } finally {
      await bundle.close();
    }
  }

  test.each<[string, Record<string, unknown>, Record<string, unknown>, RegExp]>([
    ['format cjs', {}, { format: 'cjs' }, /output\.format/],
    ['strictExecutionOrder off', {}, { strictExecutionOrder: undefined }, /strictExecutionOrder/],
    [
      'preserveEntrySignatures absent',
      { preserveEntrySignatures: undefined },
      {},
      /preserveEntrySignatures/,
    ],
    [
      'preserveEntrySignatures strict',
      { preserveEntrySignatures: 'strict' },
      {},
      /preserveEntrySignatures/,
    ],
    ['preserveModules', {}, { preserveModules: true }, /preserveModules/],
    ['onDemandWrapping', { experimental: { onDemandWrapping: true } }, {}, /onDemandWrapping/],
  ])('%s is a configuration error', async (_label, input, output, message) => {
    await expect(attempt(input, output)).rejects.toThrow(/experimentalInlineCommonChunks/);
    await expect(attempt(input, output)).rejects.toThrow(message);
  });

  test.each([-1, 1.5, Number.NaN, Number.POSITIVE_INFINITY, 2 ** 53])(
    'maxSize %p is a configuration error',
    async (maxSize) => {
      await expect(
        attempt({}, { codeSplitting: { experimentalInlineCommonChunks: { maxSize } } }),
      ).rejects.toThrow(/maxSize/);
    },
  );

  test('the same build with `maxSize: 0` succeeds', async () => {
    await attempt({}, { codeSplitting: { experimentalInlineCommonChunks: { maxSize: 0 } } });
  });
});

test('roots report errors the same way when a shared module throws', async () => {
  const pair = await buildBoth('throwing-init', {
    input: { a: './a.js', b: './b.js', main: './main.js' },
    modules: {
      './shared.js': `
        globalThis.__log('S body');
        if (globalThis.__mode === undefined) throw String(globalThis.__reason ?? 'boom');
        export const s = 1;
      `,
      './a.js': `import { s } from './shared.js'; globalThis.__log('A', s);`,
      './b.js': `import { s } from './shared.js'; globalThis.__log('B', s);`,
      './main.js': `
        for (const entry of ['./a.js', './b.js', './a.js']) {
          try { await import(entry); globalThis.__log('loaded', entry); }
          catch (e) { globalThis.__log('failed', entry, e); }
        }
      `,
    },
  });
  for (const root of ['a.js', 'b.js', 'main.js']) {
    const on = runRoot(pair.on, root);
    expect(on).toEqual(runRoot(pair.off, root));
    if (root !== 'main.js') expect(on.error).toEqual({ kind: 'value', value: 'boom:string' });
  }
});
