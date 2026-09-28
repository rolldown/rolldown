import { originalPositionFor, TraceMap } from '@jridgewell/trace-mapping';
import type { ExperimentalInlineCommonChunksOptions, Plugin } from 'rolldown';
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
  recordIds,
  runRoot,
  runRoots,
  runtimeChunk,
  virtualPlugin,
  type Built,
  type CaseOptions,
  type Modules,
} from '../../src/inline-common-chunks/harness';
import { getOutputChunk } from '../../src/utils';

// `experimentalInlineCommonChunks` prints a small common chunk's modules into the files that read
// it, except files whose static dependency outside their import cycle already prints them, instead
// of writing it as its own file. The differential cases build twice, with the option off and on,
// run every entry and dynamic entry as the root of a fresh Node process, and require the same
// logs, exports, identities and errors. The registration order check parses the `on` output:
// every `__share_require(id)` follows a `__share(id, ...)` earlier in the same file or in a static
// dependency outside the file's import cycle.

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
          /*#__NO_SIDE_EFFECTS__*/
          export function pure() { return this === undefined; }
          export const pureExpr = /*#__NO_SIDE_EFFECTS__*/ function () { return this === undefined; };
          /*#__NO_SIDE_EFFECTS__*/
          export function pureTag() { return this === undefined; }
        `,
        './a.js': `
          import * as ns from './shared.js';
          import { f, tag, K, obj, holder, pure, pureExpr, pureTag } from './shared.js';
          globalThis.__log(
            f(), f?.(), ns.f(), ns.f?.(), (ns.f)(), (ns?.f)(), ns?.f(), ns?.f?.(), ns['f'](),
            ns['f']?.(), ns?.['f'](), (ns?.f)?.(), (ns?.['f'])?.(), (ns?.tag)\`x\`,
            (ns?.['tag'])\`x\`, f.call(undefined), ns.f.call(undefined),
            Reflect.apply(ns.f, undefined, []), tag\`x\`, ns.tag\`x\`, new K().ok, new ns.K().ok,
            obj.m(), ns.obj.m(), holder.f(), (0, f)(), [f][0](),
            pure(), pure?.(), ns.pure(), ns.pure?.(), pureExpr(), ns.pureExpr(), pureTag\`x\`,
            ns.pureTag\`x\`,
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
    // The harness resolves helper names from an unminified runtime chunk, so a minified build is
    // checked on its chunk list: the record's own file is gone and both readers list its module.
    expect(pair.on.chunks.map((chunk) => chunk.name)).not.toContain('shared');
    for (const fileName of ['a.js', 'b.js']) {
      const chunk = pair.on.chunks.find((item) => item.fileName === fileName);
      expect(chunk?.moduleIds, fileName).toContain('./shared.js');
    }
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

  test('a record id follows its module set, not its code', async () => {
    // The id of the registration whose factory prints `marker`.
    const recordIdOf = (built: Built, marker: string) => {
      for (const chunk of built.chunks) {
        const parts = chunk.code.split(/\b\w+\("([^"]+)", \(\w+\) => \{/);
        for (let i = 1; i < parts.length; i += 2) {
          if (parts[i + 1].includes(marker)) return parts[i];
        }
      }
      throw new Error(`no record prints ${marker}`);
    };
    const one = `globalThis.__log('one body'); export const one = String(globalThis.__one ?? 'one');`;
    const readsOne = (name: string) =>
      `import { one } from './one/util.js'; globalThis.__log('${name}', one);`;
    const build = (name: string, input: Record<string, string>, modules: Modules) =>
      buildCase(name, 'on', { input, modules });

    const before = await build(
      'record-id-before',
      { a: './a.js', b: './b.js' },
      { './one/util.js': one, './a.js': readsOne('A'), './b.js': readsOne('B') },
    );
    const id = recordIdOf(before, 'one body');
    expect(id).toMatch(/^util-[\w-]{8}$/);

    // The id identifies the record within its build and stays put across builds while the module
    // set does; the carriers' own hashes move with the code. Files of two builds on one page are
    // not told apart by it, which is not supported.
    const edited = await build(
      'record-id-edited',
      { a: './a.js', b: './b.js' },
      {
        './one/util.js': one.replace('one body', 'one body!'),
        './a.js': readsOne('A'),
        './b.js': readsOne('B'),
      },
    );
    expect(recordIdOf(edited, 'one body!')).toBe(id);

    // Another module set under the same chunk name gets another id. `util.js` stays the record's
    // last module, so the name half stays `util` and only the hash half moves.
    const other = await build(
      'record-id-other-set',
      { a: './a.js', b: './b.js' },
      {
        './one/util.js': one,
        './one/extra.js': `export const extra = String(globalThis.__extra ?? 'extra');`,
        './a.js': `import { extra } from './one/extra.js'; import { one } from './one/util.js'; globalThis.__log('A', one, extra);`,
        './b.js': `import { extra } from './one/extra.js'; import { one } from './one/util.js'; globalThis.__log('B', one, extra);`,
      },
    );
    const otherId = recordIdOf(other, 'one body');
    expect(otherId).toMatch(/^util-[\w-]{8}$/);
    expect(otherId).not.toBe(id);
  });

  test('two records whose chunks share a name get different ids', async () => {
    // `one/util.js` and `two/util.js` each become a common chunk named `util`.
    const pair = await differential('same-named-records', {
      input: { a: './a.js', b: './b.js', c: './c.js' },
      modules: {
        './one/util.js': `
          globalThis.__log('one body');
          export const one = String(globalThis.__one ?? 'one');
        `,
        './two/util.js': `
          globalThis.__log('two body');
          export const two = String(globalThis.__two ?? 'two');
        `,
        './a.js': `import { one } from './one/util.js'; globalThis.__log('A', one);`,
        './b.js': `
          import { one } from './one/util.js';
          import { two } from './two/util.js';
          globalThis.__log('B', one, two);
        `,
        './c.js': `import { two } from './two/util.js'; globalThis.__log('C', two);`,
      },
    });
    expectInlined(pair, 'one/util.js');
    expectInlined(pair, 'two/util.js');
    expect(chunkContaining(pair.off, 'one/util.js')!.name).toBe('util');
    expect(chunkContaining(pair.off, 'two/util.js')!.name).toBe('util');
    const ids = recordIds(pair.on);
    expect(ids).toHaveLength(2);
    for (const id of ids) expect(id).toMatch(/^util-[\w-]{8}$/);
  });

  test('`maxSize: 0` is byte-identical to leaving the option out', async () => {
    const generate = async (codeSplitting: object | undefined) => {
      const bundle = await rolldown({
        input: { a: './a.js', b: './b.js' },
        preserveEntrySignatures: false,
        plugins: [virtualPlugin(twoEntries)],
      });
      const generated = await bundle.generate({
        format: 'esm',
        strictExecutionOrder: true,
        ...(codeSplitting ? { codeSplitting } : {}),
      });
      await bundle.close();
      return Object.fromEntries(
        getOutputChunk(generated).map((chunk) => [chunk.fileName, chunk.code]),
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

  test('a file inherits the factories a static dependency registers', async () => {
    // `v.js` is a manual chunk that reads `s.js`; `a.js` imports both, `b.js` only `v.js`.
    const pair = await differential('inherited-from-dependency', {
      input: { a: './a.js', b: './b.js' },
      modules: {
        './s.js': `globalThis.__log('S body'); export let n = 0; export function bump() { n += 1; return n; }`,
        './v.js': `import { bump } from './s.js'; globalThis.__log('V body'); export function viaV() { return bump(); }`,
        './a.js': `import { viaV } from './v.js'; import { n, bump } from './s.js'; globalThis.__log('A', viaV(), bump(), n);`,
        './b.js': `import { viaV } from './v.js'; globalThis.__log('B', viaV());`,
      },
      output: {
        codeSplitting: {
          groups: [{ name: 'vendor', test: /v\.js$/, includeDependenciesRecursively: false }],
        },
      },
    });
    expectInlined(pair, 's.js');
    const vendor = chunkContaining(pair.on, 'v.js')!;
    const a = chunkContaining(pair.on, 'a.js')!;
    expect(carriers(pair.on).map((chunk) => chunk.fileName)).toEqual([vendor.fileName]);
    expect(a.code).toContain('__share_require(');
    expect(a.imports).toEqual([runtimeChunk(pair.on)!.fileName, vendor.fileName]);
  });

  test('a record never reaches the filename hooks', async () => {
    const chunkNames: string[] = [];
    const sanitized: string[] = [];
    await buildCase('filename-hooks', 'on', {
      input: { a: './a.js', b: './b.js' },
      modules: twoEntries,
      output: {
        chunkFileNames(chunk) {
          chunkNames.push(chunk.name);
          return '[name].js';
        },
        sanitizeFileName(name) {
          sanitized.push(name);
          return name;
        },
      },
    });
    expect(sanitized).toContain('a');
    expect(chunkNames).not.toContain('shared');
    expect(sanitized).not.toContain('shared');
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

  test('a reader inherits a registration through a dependency a record exposes after projection', async () => {
    // `a` reads the records `r` and `s`; `r` imports the file `v` (a manual group), which reads
    // `s`. Projection makes `a` import `v`, whose registration of `s` runs before `a`'s body, so
    // `a` prints only `r`'s factory.
    const pair = await differential('inherited-through-projection', {
      input: { a: './a.js', b: './b.js', c: './c.js' },
      modules: {
        './a.js': `import { r } from './r.js'; import { s } from './s.js'; globalThis.__log('A', r(), s());`,
        './b.js': `import { r } from './r.js'; globalThis.__log('B', r());`,
        './c.js': `import { s } from './s.js'; globalThis.__log('C', s());`,
        './r.js': `import { v } from './v.js'; export function r() { return v(); }`,
        './v.js': `import { s } from './s.js'; export function v() { return s(); }`,
        './s.js': `let n = 0; globalThis.__log('S'); export function s() { return ++n; }`,
      },
      output: {
        codeSplitting: {
          groups: [{ name: 'v', test: /v\.js$/, includeDependenciesRecursively: false }],
        },
      },
    });
    const registered = (fileName: string) =>
      [
        ...getOutputChunk(pair.on.output)
          .find((chunk) => chunk.fileName === fileName)!
          .code.matchAll(/__share\("([^"]+)"/g),
      ].map((match) => match[1].slice(0, -'-XXXXXXXX'.length));
    expect(registered('a.js')).toEqual(['r']);
    expect(registered('b.js')).toEqual(['r']);
    expect(registered('c.js')).toEqual(['s']);
    expect(registered('v.js')).toEqual(['s']);
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
    // A function that rejects nothing is asked about the candidate's module and leaves it inlined.
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
    expect(seen).toContain('./shared.js');
  });
});

describe('output option combinations', () => {
  const graph: Modules = {
    './shared.js': `
      globalThis.__log('S body');
      export let count = 0;
      export const marker = { tag: String(globalThis.__tag ?? 'shared') };
      export function bump() { count += 1; return count; }
      export class Box { constructor() { this.kind = 'box'; } }
    `,
    './a.js': `
      import { bump, count, marker, Box } from './shared.js';
      globalThis.__log('A', count, bump(), count, __id(marker), new Box().kind, Box.name);
    `,
    './b.js': `
      import { bump, count, marker } from './shared.js';
      globalThis.__log('B', count, bump?.(), count, __id(marker));
    `,
  };
  const cases: Array<[string, CaseOptions]> = [
    [
      'tree shaking off',
      { input: { a: './a.js', b: './b.js' }, modules: graph, inputOptions: { treeshake: false } },
    ],
    [
      'internal export names kept',
      {
        input: { a: './a.js', b: './b.js' },
        modules: graph,
        output: { minifyInternalExports: false },
      },
    ],
    [
      'no debug info',
      {
        input: { a: './a.js', b: './b.js' },
        modules: graph,
        inputOptions: { experimental: { attachDebugInfo: 'none' } },
      },
    ],
    [
      'modules ordered by id',
      {
        input: { a: './a.js', b: './b.js' },
        modules: graph,
        inputOptions: { experimental: { chunkModulesOrder: 'module-id' } },
      },
    ],
    [
      'keepNames',
      { input: { a: './a.js', b: './b.js' }, modules: graph, output: { keepNames: true } },
    ],
  ];
  test.each(cases)('%s', async (label, options) => {
    const pair = await differential(`option-${label.replace(/\W+/g, '-')}`, options);
    expectInlined(pair, 'shared.js');
  });

  test('a carrier and its record use the same local names and import the same symbol', async () => {
    // `a.js` declares every name `shared.js` declares or imports, and imports `helper` from the
    // same vendor chunk under the same name; `two.js` imports it under another name.
    const pair = await differential('names-collide', {
      input: { a: './a.js', b: './b.js', c: './c.js' },
      modules: {
        './vendor.js': `
          export let calls = 0;
          export function helper(tag) { calls += 1; return tag + calls; }
        `,
        './shared.js': `
          import { helper } from './vendor.js';
          export let count = 0;
          export function bump() { count += 1; return helper('s') + count; }
          export const exports = String(globalThis.__x ?? 'not-the-parameter');
        `,
        './two.js': `
          import { helper as h } from './vendor.js';
          export const fromTwo = h('two');
        `,
        './a.js': `
          import { helper, calls } from './vendor.js';
          import { bump, count, exports as sharedExports } from './shared.js';
          import { fromTwo } from './two.js';
          let helper$1 = 'entry';
          helper$1 += '';
          var init_shared = 'entry-init';
          var share_shared = 'entry-bridge';
          var count$1 = 'entry-count';
          globalThis.__log('A', bump(), count, helper('a'), calls, fromTwo, sharedExports, helper$1, init_shared, share_shared, count$1);
        `,
        './b.js': `import { bump, count } from './shared.js'; import { fromTwo } from './two.js'; globalThis.__log('B', bump(), count, fromTwo);`,
        './c.js': `import { fromTwo } from './two.js'; globalThis.__log('C', fromTwo);`,
      },
      output: {
        codeSplitting: {
          groups: [{ name: 'vendor', test: /vendor\.js$/, includeDependenciesRecursively: false }],
        },
      },
    });
    expectInlined(pair, 'shared.js');
    expectInlined(pair, 'two.js');
    const a = chunkContaining(pair.on, 'a.js')!;
    // One import declaration from the vendor chunk serves the file and both factories.
    expect(a.code.match(/from "\.\/vendor[^"]*";/g)).toHaveLength(1);
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
      // A carrier names its own modules and the record's in one pass; a binding a module reads
      // through direct `eval` must keep its source name, so the record stays a file.
      'a chunk that would print it holding a module with direct eval',
      {
        input: { a: './a.js', b: './b.js' },
        modules: {
          './shared.js': `export const s = String(globalThis.__s ?? 'S'); export var config = { from: 'shared' };`,
          './probe.js': `var config = { from: 'probe' }; export const probed = eval('config.from');`,
          './a.js': `import { probed } from './probe.js'; import { s } from './shared.js'; globalThis.__log('A', s, probed);`,
          './b.js': `import { s } from './shared.js'; globalThis.__log('B', s);`,
        },
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
    [
      'a member re-exports an external module',
      {
        input: { a: './a.js', b: './b.js' },
        modules: {
          './barrel.js': `export * from '#ext/lib'; export const own = 1;`,
          './a.js': `import { x, own } from './barrel.js'; globalThis.__log('A', x, own);`,
          './b.js': `import { y } from './barrel.js'; globalThis.__log('B', y);`,
        },
        inputOptions: { external: [/^#ext\//] },
        files: {
          'ext-lib.js': `globalThis.__log('ext-lib'); export const x = 'X'; export const y = 'Y';`,
        },
      },
      'barrel.js',
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
      plugins: [virtualPlugin(modules)],
      preserveEntrySignatures: false,
      ...input,
    });
    try {
      const generated = await bundle.generate({
        format: 'esm',
        strictExecutionOrder: true,
        codeSplitting: { experimentalInlineCommonChunks: inline },
        ...output,
      });
      return getOutputChunk(generated);
    } finally {
      await bundle.close();
    }
  }

  test.each<[string, Record<string, unknown>, Record<string, unknown>, RegExp]>([
    ['format cjs', {}, { format: 'cjs' }, /output\.format/],
    ['strictExecutionOrder false', {}, { strictExecutionOrder: false }, /strictExecutionOrder/],
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
    ['intro', {}, { intro: 'const __STATE__ = [];' }, /intro/],
    ['outro', {}, { outro: () => 'const __CFG__ = 1;' }, /outro/],
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

  test('an omitted strictExecutionOrder is turned on', async () => {
    const chunks = await attempt({}, { strictExecutionOrder: undefined });
    expect(chunks.map((chunk) => chunk.code).join('\n')).toContain('__share');
  });

  test('`maxSize: 0` skips the preconditions', async () => {
    await attempt(
      {},
      { format: 'cjs', codeSplitting: { experimentalInlineCommonChunks: { maxSize: 0 } } },
    );
  });
});

test('assigning to a property of a CommonJS default export read through a record', async () => {
  // `m.js` re-exports the default of a CommonJS module and becomes a record. Every assignment
  // form to a property of that export is rewritten to `ns.default.prop`, and `ns` must be the
  // bridge, in the entry that carries the factory and in a file that only requires the record.
  const pair = await differential('cjs-default-assignment', {
    input: { a: './a.js', b: './b.js', c: './c.js' },
    modules: {
      './lib.cjs': `module.exports = { name: 'lib', count: 0 };`,
      './m.js': `import lib from './lib.cjs'; export { lib }; export const tag = 'm';`,
      './a.js': `
        import { lib, tag } from './m.js';
        lib.count = 1;
        lib.count += 1;
        lib.count++;
        lib['flag'] = 'a';
        globalThis.__log('A', tag, lib.name, lib.count, lib.flag);
      `,
      './b.js': `
        import { lib } from './m.js';
        lib.flag = 'b';
        globalThis.__log('B', lib.name, lib.count, lib.flag);
      `,
      './c.js': `
        import { lib } from './m.js';
        import { tag } from './m.js';
        function bump() { lib.count += 10; return lib.count; }
        globalThis.__log('C', tag, bump(), lib.flag);
      `,
    },
  });
  expectInlined(pair, 'm.js');
});

test('a reader that inherits the registration writes to the record, not to its own same-named binding', async () => {
  // `f.js` gets the registration of the `m.js` record from its static dependency `d.js` and owns
  // an unrelated `import_lib` for `y/lib.cjs`. Its assignment to `lib.flag` must reach the record's
  // `x/lib.cjs`, so `other.flag` stays undefined.
  const pair = await differential('inherited-reader-assignment', {
    input: { f: './f.js', d: './d.js', b: './b.js' },
    modules: {
      './x/lib.cjs': `module.exports = { name: 'x-lib' };`,
      './y/lib.cjs': `module.exports = { name: 'y-lib' };`,
      './m.js': `import lib from './x/lib.cjs'; export { lib };`,
      './d.js': `import { lib } from './m.js'; export const dname = lib.name; globalThis.__log('d', dname);`,
      './f.js': `
        import { dname } from './d.js';
        import { lib } from './m.js';
        import other from './y/lib.cjs';
        lib.flag = 'set-by-f';
        globalThis.__log('f', dname, lib.flag, other.flag, other.name);
      `,
      './b.js': `import { lib } from './m.js'; globalThis.__log('b', lib.name, lib.flag);`,
    },
  });
  expectInlined(pair, 'm.js');
  const f = runRoots(pair.on, ['f.js']);
  expect(f.logs).toContainEqual(
    expect.stringMatching(/^f:string x-lib:string set-by-f:string undefined y-lib:string$/),
  );
});

test('a lazy route that re-exports a CommonJS binding keeps the CommonJS chunk a file', async () => {
  // Under strict execution order the interop symbols for `widget.cjs` (the `init_*_cjs_*` wrapper
  // and the namespace) are owned by the importing route but declared in the CommonJS chunk. The
  // route that re-exports one of them cannot forward a bridge as a live binding, so the chunk
  // that declares them stays a file.
  const pair = await differential('lazy-route-cjs-reexport', {
    input: { main: './main.js' },
    modules: {
      './main.js': `
        const routes = { a: () => import('./routeA.js'), b: () => import('./routeB.js') };
        globalThis.__done = (async () => {
          const b = await routes.b();
          globalThis.__log('b', b.render());
          const a = await routes.a();
          globalThis.__log('a', a.default());
        })();
        await globalThis.__done;
      `,
      './routeA.js': `export { default } from './widget.cjs';`,
      './routeB.js': `import widget from './widget.cjs'; export function render() { return 'B:' + widget(); }`,
      './widget.cjs': `module.exports = function widget() { return 'widget'; };`,
    },
  });
  expect(chunkContaining(pair.on, 'widget.cjs')).toBeDefined();
});

test('a record export named `__proto__` is a getter, not the prototype', async () => {
  const pair = await differential('proto-export', {
    input: { a: './a.js', b: './b.js' },
    modules: {
      './shared.js': `
        export let __proto__ = String(globalThis.__v ?? 'P');
        export const other = String(globalThis.__o ?? 'O');
      `,
      './a.js': `
        import { __proto__ as proto, other } from './shared.js';
        globalThis.__log('A', proto, other);
      `,
      './b.js': `
        import { __proto__ as proto, other } from './shared.js';
        globalThis.__log('B', proto, other);
      `,
    },
    output: { minifyInternalExports: false },
  });
  expectInlined(pair, 'shared.js');
  const carrier = pair.on.chunks.find((chunk) => chunk.fileName === 'a.js')!;
  expect(carrier.code).toContain('["__proto__"]: () =>');
});

test('a CommonJS local named like the bridge leaves the bridge intact', async () => {
  const pair = await differential('cjs-local-shadows-bridge', {
    input: { a: './a.js', b: './b.js' },
    modules: {
      './a.js': `import c from './c.js'; globalThis.__log('A', c.v, c.local);`,
      './b.js': `import { count } from './shared.js'; globalThis.__log('B', count);`,
      './c.js': `
        var share_shared = 'local';
        const s = require('./shared.js');
        function g() { return s.count; }
        exports.v = g() + ':' + s.bump();
        exports.local = share_shared;
      `,
      './shared.js': `export let count = 1; export function bump() { count++; return count; }`,
    },
  });
  expectInlined(pair, 'shared.js');
  const carrier = pair.on.chunks.find((chunk) => chunk.fileName === 'a.js')!;
  expect(carrier.code).toMatch(/var share_shared\$\d+ = /);
});

test('a CommonJS module inside a record keeps its local apart from the factory bridge', async () => {
  const pair = await differential('cjs-local-shadows-factory-bridge', {
    input: { a: './a.js', b: './b.js', c: './c.js' },
    modules: {
      './a.js': `
        import m from './mid.cjs';
        import { count } from './shared.js';
        globalThis.__log('A', m.v, count);
      `,
      './b.js': `import m from './mid.cjs'; globalThis.__log('B', m.v);`,
      './c.js': `
        import { count, bump } from './shared.js';
        function h() { let share_shared = 'inner'; return count + share_shared; }
        globalThis.__log('C', bump(), h());
      `,
      './mid.cjs': `
        var share_shared = 'local';
        const s = require('./shared.js');
        exports.v = s.bump() + ':' + share_shared;
      `,
      './shared.js': `export let count = 1; export function bump() { count++; return count; }`,
    },
  });
  expectInlined(pair, 'mid.cjs');
  expectInlined(pair, 'shared.js');
});

// Two builds publish into one directory and a page loads a root from each. Their runtime chunks
// have the same content, so the same hashed name, and with the option on one registry serves both
// builds: a record's id must therefore tell the two builds' records apart whenever a hashed file
// name would, or the second build's consumers would run the first build's factory.
test('an edit inside one carrier leaves the record id and every other hashed file alone', async () => {
  // `shared/utils.js` is a record carried by `a`, `big` and `p2`. The second version adds
  // `pages/utils.js` to `a` only, whose `init_utils` makes `a` rename the record's own `init_utils`
  // in its copy. The record's id must not follow one carrier's names, and a hashed file that keeps
  // its name must keep its bytes, or a cached copy from the first version would run beside files
  // of the second.
  const v1: Modules = {
    './shared/utils.js': `globalThis.__log('lib body'); let count = 0; export function bump() { count += 1; return count; }`,
    './big.js': `import { bump } from './shared/utils.js'; globalThis.__log('big body'); export function viaBig() { return 'big:' + bump(); }`,
    './a.js': `import { bump } from './shared/utils.js'; globalThis.__log('A', bump());`,
    './p1.js': `import { viaBig } from './big.js'; globalThis.__log('P1', viaBig());`,
    './p2.js': `import { viaBig } from './big.js'; import { bump } from './shared/utils.js'; globalThis.__log('P2', viaBig(), bump());`,
  };
  const v2: Modules = {
    ...v1,
    './pages/utils.js': `globalThis.__log('ui body'); export const label = String(globalThis.__l ?? 'ui');`,
    './a.js': `import { bump } from './shared/utils.js'; import { label } from './pages/utils.js'; globalThis.__log('A', bump(), label);`,
  };
  const build = (name: string, modules: Modules) =>
    buildCase(name, 'on', {
      input: { a: './a.js', p1: './p1.js', p2: './p2.js' },
      modules,
      inline: { maxSize: 1 << 20, exclude: /big\.js$/ },
      output: { entryFileNames: '[name].js', chunkFileNames: '[name]-[hash].js' },
    });
  const first = await build('carrier-edit-1', v1);
  const second = await build('carrier-edit-2', v2);
  const codeByName = (built: Built) =>
    new Map(built.chunks.map((chunk) => [chunk.fileName, chunk.code]));
  const firstCode = codeByName(first);
  const secondCode = codeByName(second);

  // The record keeps its id although `a` prints its copy with different names.
  expect(recordIds(second)).toEqual(recordIds(first));
  const [id] = recordIds(first);
  const factoryIn = (code: string) => {
    const start = code.indexOf(`("${id}", (`);
    expect(start).toBeGreaterThan(0);
    return code.slice(start, code.indexOf('__share_export(', start));
  };
  expect(factoryIn(secondCode.get('a.js')!)).not.toBe(factoryIn(firstCode.get('a.js')!));

  // Every hashed file present in both versions is byte for byte the same file.
  for (const [name, code] of firstCode) {
    if (/-[\w-]{8}\.js$/.test(name) && secondCode.has(name)) {
      expect(secondCode.get(name), name).toBe(code);
    }
  }
  expect([...firstCode.keys()].filter((name) => name.startsWith('big-'))).toEqual(
    [...secondCode.keys()].filter((name) => name.startsWith('big-')),
  );
});

test('files behind a record keep their place in the import order, externals included', async () => {
  // `x` and `c` import an external module each; `r` is the record and imports `c`. With the
  // option off, e2 evaluates x's external, then r's dependency c and its external; the on build
  // must not move c ahead of x.
  const ext = (name: string) => `globalThis.__log('${name}');\n`;
  const pair = await differential('external-order', {
    input: { e1: './e1.js', e2: './e2.js', e3: './e3.js', e4: './e4.js' },
    modules: {
      './e1.js': `import './c.js'; globalThis.__log('e1');`,
      './e2.js': `import './x.js'; import './r.js'; globalThis.__log('e2');`,
      './e3.js': `import './r.js'; globalThis.__log('e3');`,
      './e4.js': `import './x.js'; globalThis.__log('e4');`,
      './c.js': `import '#ext/c'; globalThis.__log('c');`,
      './r.js': `import './c.js'; globalThis.__log('r');`,
      './x.js': `import '#ext/x'; globalThis.__log('x');`,
    },
    inputOptions: { external: (id: string) => id.startsWith('#ext/') },
    files: { 'ext-c.js': ext('ext-c'), 'ext-x.js': ext('ext-x') },
  });
  expectInlined(pair, 'r.js');
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
