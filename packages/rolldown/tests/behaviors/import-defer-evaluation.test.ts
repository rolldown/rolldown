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
  expectedChunks?: number,
) {
  const cwd = await mkdtemp(path.join(tmpdir(), 'rolldown-import-defer-'));
  onTestFinished(() => rm(cwd, { recursive: true, force: true }));
  for (const [name, source] of Object.entries(files)) {
    const file = path.join(cwd, name);
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, source);
  }
  const ext = output.format === 'cjs' ? 'cjs' : 'mjs';
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
    const result = await build.write({
      dir: 'out',
      entryFileNames: `[name].${ext}`,
      chunkFileNames: `[name]-[hash].${ext}`,
      ...output,
    });
    if (expectedChunks !== undefined) {
      expect(result.output.filter((item) => item.type === 'chunk')).toHaveLength(expectedChunks);
    }
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
    execFileSync(process.execPath, [...args, `out/${entry}.${ext}`], {
      cwd,
      encoding: 'utf8',
      stdio: 'pipe',
      timeout: 10_000,
    })
      .trim()
      .split(/\r?\n/);
}

const waitsForRelease =
  "console.log('a start'); await new Promise(r => globalThis.release = r); console.log('a end');";

const cjsOutputs = [
  { format: 'esm', strictExecutionOrder: true },
  { format: 'cjs', preserveModules: true, generatedCode: { profilerNames: true } },
] satisfies OutputOptions[];

test.each(cjsOutputs)('reentrant reads of shared CommonJS exports, %j', async (output) => {
  const importer = `import defer * as ns from './target.cjs';
    export { ns };
    export const read = () => ns.default.value;
    export const has = () => 'value' in ns;`;
  const run = await bundle(
    {
      'first.mjs': importer,
      'second.mjs': importer,
      'target.cjs': `exports.early = true;
      globalThis.inspect();
      if (globalThis.replace) module.exports = {};
      module.exports.value = 1;`,
      'start.cjs': "exports.start = () => require('./target.cjs').value;",
      'main.mjs': `import * as first from './first.mjs';
      import * as second from './second.mjs';
      import starter from './start.cjs';
      const a = first.ns, b = second.ns;
      console.log('same', a === b);
      globalThis.inspect = () => {
        console.log('inner', second.read(), second.has(), b.default.early, Object.keys(b).join(','));
      };
      const value = globalThis.start === 'require' ? starter.start()
        : globalThis.start === 'proxy' ? a.default.value : first.read();
      console.log('outer', value, second.read(), second.has(), b.default.value, Object.keys(a).join(','));
    `,
    },
    output,
  );
  for (const start of ['read', 'proxy', 'require']) {
    for (const replace of [false, true]) {
      const setup = `data:text/javascript,globalThis.start = '${start}'; globalThis.replace = ${replace}`;
      expect(run('main', ['--import', setup])).toEqual([
        'same true',
        'inner undefined false true default,early',
        `outer 1 1 true 1 ${replace ? 'default,value' : 'default,early,value'}`,
      ]);
    }
  }
});

test('a single deferred CommonJS namespace reads replaced exports after a callback', async () => {
  const run = await bundle({
    'a.mjs': "import defer * as ns from './target.cjs'; export { ns };",
    'target.cjs': 'exports.value = 0; globalThis.observe(); module.exports = { value: 1 };',
    'main.mjs': `import { ns } from './a.mjs';
      globalThis.observe = () => console.log('inner', ns.default.value);
      console.log('outer', ns.default.value);`,
  });
  expect(run()).toEqual(['inner 0', 'outer 1']);
});

test.each(cjsOutputs)('locking shared CommonJS exports during evaluation, %j', async (output) => {
  const importer = `import defer * as ns from './target.cjs';
    export { ns };
    export const read = () => [ns.value, ns.toString, 'late' in ns];`;
  const run = await bundle(
    {
      'a.mjs': importer,
      'b.mjs': importer,
      'target.cjs': `exports.value = 1;
      if (globalThis.replace) exports.toString = 7;
      globalThis.observe(exports);
      if (globalThis.replace) module.exports = { value: 2, late: 3 };`,
      'main.mjs': `import { ns as a, read } from './a.mjs';
      import { ns as b } from './b.mjs';
      globalThis.observe = (exports) => {
        console.log('inner', globalThis.staticRead ? read()[0] : b.default.value);
        if (globalThis.replace) exports.beforeLock = true;
        console.log('lock', Reflect.preventExtensions(b), Object.isExtensible(a));
      };
      console.log('outer', a.default.value, Object.keys(b).join(','));
      if (globalThis.replace) {
        console.log('static', ...read());
        console.log('proxy', a.value, a.toString, 'late' in b, a.default.late);
        console.log('descriptor', Object.getOwnPropertyDescriptor(a, 'toString').value);
      }`,
    },
    output,
  );
  expect(run()).toEqual(['inner 1', 'lock true false', 'outer 1 default,value']);
  for (const staticRead of [false, true]) {
    const setup = `data:text/javascript,globalThis.replace=true;globalThis.staticRead=${staticRead}`;
    expect(run('main', ['--import', setup])).toEqual([
      'inner 1',
      'lock true false',
      'outer 2 beforeLock,default,toString,value',
      'static 2 undefined false',
      'proxy 2 undefined false 3',
      'descriptor undefined',
    ]);
  }
});

test.each(cjsOutputs)(
  'locking a shared CommonJS namespace never probes its exports, %j',
  async (output) => {
    const importer = "import defer * as ns from './target.cjs'; export { ns };";
    const run = await bundle(
      {
        'a.mjs': importer,
        'b.mjs': importer,
        'start.cjs': "module.exports = () => require('./target.cjs');",
        'target.cjs': `const { proxy, revoke } = Proxy.revocable({ value: 1 }, {
      getPrototypeOf(t) { globalThis.probes++; return Reflect.getPrototypeOf(t); },
      ownKeys(t) { globalThis.probes++; return Reflect.ownKeys(t); }
    });
    if (globalThis.accessor) Object.defineProperty(module, 'exports', {
      get() { globalThis.probes++; return proxy; }
    });
    else module.exports = proxy;
    globalThis.revoke = revoke;
    if (globalThis.during) globalThis.lock();`,
        'main.mjs': `import { ns as a } from './a.mjs';
      import { ns as b } from './b.mjs';
      import start from './start.cjs';
      console.log('same', a === b);
      globalThis.probes = 0;
      globalThis.lock = () => {
        if (globalThis.seen) void a.default;
        if (globalThis.revoked) globalThis.revoke();
        globalThis.probes = 0;
        console.log('lock', Reflect.preventExtensions(b), Object.isExtensible(a), globalThis.probes);
      };
      start();
      if (!globalThis.during) globalThis.lock();
      try { console.log('read', Object.keys(a).join(','), b.default.value); }
      catch (error) { console.log('read', error.name); }`,
      },
      output,
    );
    for (const during of [false, true]) {
      for (const { result, ...flags } of [
        { revoked: true, result: 'TypeError' },
        { result: 'default 1' },
        { seen: true, result: 'default,value 1' },
        { accessor: true, result: 'default 1' },
      ]) {
        const setup = `data:text/javascript,Object.assign(globalThis,${JSON.stringify({ during, ...flags })})`;
        expect(run('main', ['--import', setup])).toEqual([
          'same true',
          'lock true false 0',
          `read ${result}`,
        ]);
      }
    }
  },
);

test.each(cjsOutputs)(
  'locking an unread CommonJS literal preserves its live keys, %j',
  async (output) => {
    const importer = "import defer * as ns from './target.cjs'; export { ns };";
    const run = await bundle(
      {
        'a.mjs': importer,
        'b.mjs': importer,
        'start.cjs': "module.exports = () => require('./target.cjs');",
        'target.cjs': `module.exports = { get x() { console.log('getter'); return 1; } };`,
        'main.mjs': `import { ns as a } from './a.mjs';
      import { ns as b } from './b.mjs';
      import start from './start.cjs';
      const value = start();
      value.beforeLock = 2;
      console.log('lock', Reflect.preventExtensions(a));
      value.afterLock = 3;
      const names = Reflect.ownKeys(b).filter(key => typeof key === 'string');
      console.log(names.join(','), b.x, b.beforeLock, b.afterLock, b.default.afterLock);`,
      },
      output,
    );
    const expected = ['lock true', 'getter', 'beforeLock,default,x 1 2 undefined 3'];
    expect(run()).toEqual(expected);
    expect(
      run('main', ['--import', 'data:text/javascript,Object.preventExtensions=undefined']),
    ).toEqual(expected);
  },
);

test.each(cjsOutputs)(
  'locking an unread CommonJS export from a local binding, %j',
  async (output) => {
    const importer =
      "import defer * as ns from './target.cjs'; export { ns }; export const read = () => ns.value;";
    const run = await bundle(
      {
        'a.mjs': importer,
        'b.mjs': importer,
        'start.cjs': "module.exports = () => require('./target.cjs');",
        'target.cjs': `const obj = { value: 1 };
      const arrow = () => ({ value: 2 });
      function makeExports() {
        if (globalThis.source === 'proxy') return new Proxy(obj, {
          ownKeys(target) { globalThis.probes++; return Reflect.ownKeys(target); }
        });
        return { value: 3 };
      }
      if (globalThis.source === 'alias') module.exports = obj;
      else if (globalThis.source === 'arrow') module.exports = arrow();
      else module.exports = makeExports();`,
        'main.mjs': `import { ns as a } from './a.mjs'; import { ns as b, read } from './b.mjs';
      import start from './start.cjs';
      globalThis.probes = 0;
      start();
      console.log('lock', Reflect.preventExtensions(a), globalThis.probes);
      console.log('read', b.value, read(), b.default.value);`,
      },
      output,
    );
    for (const [source, expected] of [
      ['alias', '1 1 1'],
      ['arrow', '2 2 2'],
      ['factory', '3 3 3'],
      ['proxy', 'undefined undefined 1'],
    ]) {
      expect(
        run('main', ['--import', `data:text/javascript,globalThis.source='${source}'`]),
      ).toEqual(['lock true 0', `read ${expected}`]);
    }
  },
);

test('locking a second target that exports the same proxy uses its own observations', async () => {
  const a = "import defer * as ns from './one.cjs'; export { ns };";
  const b = "import defer * as ns from './two.cjs'; export { ns };";
  const run = await bundle({
    'a.mjs': a,
    'b.mjs': a,
    'c.mjs': b,
    'd.mjs': b,
    'one.cjs': 'module.exports = globalThis.shared; globalThis.inspect();',
    'two.cjs': 'module.exports = globalThis.shared;',
    'start.cjs': "exports.start = () => { require('./one.cjs'); require('./two.cjs'); };",
    'main.mjs': `import { ns as a } from './a.mjs'; import { ns as b } from './b.mjs';
      import { ns as c } from './c.mjs'; import { ns as d } from './d.mjs';
      import start from './start.cjs';
      globalThis.shared = new Proxy({ x: 1 }, {});
      globalThis.inspect = () => console.log(a === b, Object.keys(a).join(','));
      start.start();
      console.log(c === d, Reflect.preventExtensions(c), Object.keys(d).join(','), d.default.x);`,
  });
  expect(run()).toEqual(['true default,x', 'true true default 1']);
});

test.each(cjsOutputs)('lazy CommonJS reads use the captured Object.create, %j', async (output) => {
  const importer =
    "import defer * as ns from './target.cjs'; export const read = () => ns.default.value;";
  const run = await bundle(
    {
      'a.mjs': importer,
      'b.mjs': importer,
      'target.cjs': 'module.exports = { value: 1 };',
      'main.mjs': `import { read as a } from './a.mjs';
      import { read as b } from './b.mjs';
      const original = Object.create;
      Object.create = () => { throw new Error('unexpected Object.create'); };
      try { console.log(a(), b()); } finally { Object.create = original; }`,
    },
    output,
  );
  expect(run()).toEqual(['1 1']);
  expect(run('main', ['--import', 'data:text/javascript,globalThis.WeakMap=undefined'])).toEqual([
    '1 1',
  ]);
});

test.each(cjsOutputs)(
  'reading an ESM namespace without Reflect.preventExtensions, %j',
  async (output) => {
    const run = await bundle(
      {
        'main.mjs': `import defer * as ns from './target.mjs';
      function read(object) { console.log('value', object.x); }
      read(ns);`,
        'target.mjs': 'export const x = 1;',
      },
      output,
    );
    expect(
      run('main', ['--import', 'data:text/javascript,Reflect.preventExtensions=undefined']),
    ).toEqual(['value 1']);
  },
);

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

test.each([
  { source: "import './bad.mjs'; await 0;", stdout: 'bad\n' },
  { source: "if (globalThis.Promise) throw new Error('boom'); await 0;", stdout: 'b\n' },
  { source: "await 0; throw new Error('boom');", stdout: 'b\n' },
])('a combined wait preserves the origin of a throw in $source', async ({ source, stdout }) => {
  const run = await bundle({
    'main.mjs':
      "import defer * as a from './a.mjs'; import defer * as b from './b.mjs'; console.log('main', typeof a, typeof b);",
    'a.mjs': source,
    'bad.mjs': "console.log('bad'); throw new Error('boom');",
    'b.mjs': "console.log('b'); await 0;",
  });
  expect(() => run()).toThrow(
    expect.objectContaining({ status: 1, stdout, stderr: expect.stringContaining('Error: boom') }),
  );
});

test.each([false, true])(
  'dynamic defer evaluates the gathered roots independently, code splitting %s',
  async (codeSplitting) => {
    for (const ownAwait of [false, true]) {
      const run = await bundle(
        {
          'main.mjs':
            "try { await import.defer('./target.mjs'); } catch (e) { console.log('caught', e.message); }",
          'target.mjs': ownAwait
            ? "import './bad.mjs'; import './b.mjs'; await 0; export const x = 1;"
            : "import './a.mjs'; import './b.mjs'; export const x = 1;",
          'a.mjs': "import './bad.mjs'; await 0;",
          'bad.mjs': "console.log('bad'); throw new Error('boom');",
          'b.mjs': "console.log('b'); await 0;",
        },
        { codeSplitting },
      );
      expect(run()).toEqual(ownAwait ? ['bad', 'caught boom'] : ['bad', 'b', 'caught boom']);
    }
  },
);

test('a pending dynamic root lets later roots start and reject independently', async () => {
  const run = await bundle({
    'main.mjs': `
      globalThis.failure = {};
      try { await import.defer('./target.mjs'); } catch (e) { console.log(e === globalThis.failure); }
    `,
    'target.mjs': "import './pending.mjs'; import './a.mjs'; import './b.mjs';",
    'pending.mjs': "console.log('pending'); await new Promise(() => {});",
    'a.mjs': "import './bad.mjs'; await 0;",
    'bad.mjs': "console.log('bad'); throw globalThis.failure;",
    'b.mjs': "console.log('b'); await 0; throw new Error('later');",
  });
  expect(run('main', ['--unhandled-rejections=strict'])).toEqual(['pending', 'bad', 'b', 'true']);
});

test('a synchronous failure stops the walk while an earlier request is pending', async () => {
  const run = await bundle({
    'main.mjs': `
      globalThis.failure = {};
      try { await import('./requests.mjs'); } catch (error) { console.log(error === globalThis.failure); }
    `,
    'requests.mjs': `
      import defer * as first from './pending.mjs';
      import defer * as second from './async.mjs';
      import defer * as last from './last.mjs';
      console.log('unreachable');
    `,
    'pending.mjs': "console.log('pending'); await new Promise(() => {});",
    'async.mjs': "export * from './bad.mjs'; await 0;",
    'bad.mjs': "console.log('bad'); throw globalThis.failure;",
    'last.mjs': "console.log('last'); await 0;",
  });
  expect(run()).toEqual(['pending', 'bad', 'true']);
});

test.each([
  'shared request',
  'shared request with an async parent',
  'unmatched group',
  'eager request between deferred imports',
])('an eager fallback starts async siblings in source order with a %s', async (shape) => {
  const interleaved = shape === 'eager request between deferred imports';
  const shared = shape.startsWith('shared request');
  const run = await bundle(
    {
      'main.mjs': `import defer * as ns from './target.mjs'; ${interleaved ? "import './middle.mjs';" : ''} import defer * as extra from './extra.mjs'; console.log('main', ns.x, typeof extra);`,
      'other.mjs': `${shared ? "import './extra.mjs';" : ''} console.log('other');`,
      'target.mjs': `import './a.mjs'; import defer * as b from './b.mjs'; ${shape === 'shared request with an async parent' ? 'await 0;' : ''} export const x = 1;`,
      'a.mjs': waitsForRelease,
      'b.mjs': "console.log('b start'); globalThis.release(); await 0; console.log('b end');",
      'extra.mjs': 'await 0; export const x = 1;',
      'middle.mjs': "console.log('middle');",
    },
    shape === 'unmatched group'
      ? { codeSplitting: { groups: [{ name: 'deps', test: /missing/ }] } }
      : {},
    ['main', 'other'],
    interleaved ? 'comes after a deferred import' : true,
  );
  expect(run()).toEqual([
    'a start',
    'b start',
    ...(interleaved ? ['middle'] : []),
    'a end',
    'b end',
    'main 1 object',
  ]);
  expect(run('other')).toEqual(['other']);
});

test.each([
  [1, false],
  [1, true],
  [64, false],
  [64, true],
] as const)(
  '%i consumers of one async dependency share one file, minify %s',
  async (count, minify) => {
    const consumers = Array.from({ length: count }, (_, i) => `consumer-${i}.mjs`);
    const run = await bundle(
      {
        'main.mjs':
          "import defer * as a from './a.mjs'; import './boot.mjs'; console.log('main', a.value);",
        'a.mjs':
          "console.log('start'); await 0; console.log('end'); queueMicrotask(() => console.log('settled')); export const value = 1;",
        'boot.mjs': consumers.map((file) => `import './${file}';`).join('\n'),
        ...Object.fromEntries(
          consumers.map((file, i) => [
            file,
            `import { value } from './a.mjs'; console.log('consumer', ${i}, value); ${i === 0 ? "queueMicrotask(() => console.log('consumer microtask'));" : ''}`,
          ]),
        ),
      },
      { minify },
      ['main'],
      'comes after a deferred import',
      2,
    );
    const expected = [
      'start',
      'end',
      'settled',
      ...consumers.map((_, i) => `consumer ${i} 1`),
      'main 1',
    ];
    expect(run()).toEqual([...expected, 'consumer microtask']);
    expect(
      run('main', [
        '--input-type=module',
        '-e',
        "import './out/main.mjs'; console.log('external importer')",
      ]),
    ).toEqual([...expected, 'external importer', 'consumer microtask']);
  },
);

test('keepNames restores function names inside async sibling scopes', async () => {
  const consumer = "import './a.mjs'; console.log(named.name); function named() {}";
  const run = await bundle(
    {
      'main.mjs': "import defer * as a from './a.mjs'; import './join.mjs'; console.log(typeof a);",
      'a.mjs': 'await 0;',
      'one.mjs': consumer,
      'two.mjs': consumer,
      'join.mjs': "import './one.mjs'; import './two.mjs';",
    },
    { keepNames: true, minify: true },
    ['main'],
    'comes after a deferred import',
    3,
  );
  expect(run()).toEqual(['named', 'named', 'object']);
});

test('a throw before a shared await stops an external importer’s later request', async () => {
  const run = await bundle(
    {
      'main.mjs':
        "import defer * as a from './a.mjs'; import './consumer.mjs'; console.log(typeof a);",
      'a.mjs': "import './bad.mjs'; await 0;",
      'bad.mjs': "console.log('bad'); throw new Error('boom');",
      'consumer.mjs': "import './a.mjs'; console.log('consumer');",
      'later.mjs': "console.log('later');",
    },
    {},
    ['main'],
    'comes after a deferred import',
    3,
  );
  expect(() =>
    run('main', ['--input-type=module', '-e', "import './out/main.mjs'; import './later.mjs';"]),
  ).toThrow(
    expect.objectContaining({
      status: 1,
      stdout: 'bad\n',
      stderr: expect.stringContaining('Error: boom'),
    }),
  );
});

test('a shared await resumes ready siblings even when the first throws', async () => {
  const run = await bundle(
    {
      'main.mjs':
        "import defer * as a from './a.mjs'; import './join.mjs'; console.log('main', typeof a);",
      'a.mjs': "await 0; console.log('a');",
      'first.mjs': "import './a.mjs'; console.log('first'); throw undefined;",
      'second.mjs':
        "import './a.mjs'; console.log('second', typeof arguments); throw Error('second');",
      'join.mjs': "import './first.mjs'; import './second.mjs'; console.log('join');",
    },
    {},
    ['main'],
    'comes after a deferred import',
    2,
  );
  expect(
    run('main', [
      '--input-type=module',
      '-e',
      "await import('./out/main.mjs').catch(e => console.log('caught', e))",
    ]),
  ).toEqual(['a', 'first', 'second undefined', 'caught undefined']);
});

test.each([false, true])(
  'a synchronous sibling releases the only top-level await, separate wait chunk %s',
  async (separateWait) => {
    const run = await bundle(
      {
        'main.mjs':
          "import defer * as a from './a.mjs'; import './release.mjs'; console.log('main', typeof a);",
        'a.mjs': waitsForRelease,
        'release.mjs': "console.log('release'); globalThis.release();",
      },
      separateWait ? { codeSplitting: { groups: [{ name: 'wait', test: /[\\/]a\.mjs$/ }] } } : {},
      ['main'],
      'comes after a deferred import',
    );
    expect(run()).toEqual(['a start', 'release', 'a end', 'main object']);
  },
);

test('a synchronous release reached through a re-export can start in another entry', async () => {
  const run = await bundle(
    {
      'main.mjs': "import defer * as x from './x.mjs'; import './y.mjs'; console.log(typeof x);",
      'other.mjs': "import './release.mjs'; console.log('other');",
      'x.mjs': "export * from './wait.mjs';",
      'y.mjs': "export * from './release.mjs';",
      'wait.mjs': waitsForRelease,
      'release.mjs': "console.log('release'); globalThis.release?.();",
    },
    { codeSplitting: { groups: [{ name: 'deps', test: /[\\/](wait|release)\.mjs$/ }] } },
    ['main', 'other'],
    'comes after a deferred import',
  );
  expect(run()).toEqual(['a start', 'release', 'a end', 'object']);
  expect(run('other')).toEqual(['release', 'other']);
});

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

test.each(['then', 'other'])('dynamic imports of a target exporting %s', async (name) => {
  const sources: Record<string, string> = {
    'main.mjs':
      "export const load = () => import.defer('target.mjs'); export const eager = () => import('target.mjs'); export const extra = () => import('extra.mjs');",
    'second.mjs': "export const load = () => import.defer('target.mjs');",
    'extra.mjs': 'export const extraValue = 1;',
    'target.mjs': `export function ${name}() {}`,
  };
  const build = await rolldown({
    input: ['main.mjs', 'second.mjs'],
    experimental: { importDefer: true },
    plugins: [
      {
        name: 'sources',
        resolveId: (id) => (id in sources ? id : null),
        load: (id) => sources[id],
      },
    ],
  });
  try {
    const { output } = await build.generate({});
    const chunks = output.filter((item) => item.type === 'chunk');
    const entries = chunks.filter((chunk) => chunk.isEntry);
    const target = chunks.find((chunk) => chunk.exports.includes(name))!;
    const main = entries.find((entry) => entry.exports.includes('eager'))!;
    const second = entries.find((entry) => entry !== main)!;
    const destination = chunks.find((chunk) => chunk.fileName === second.dynamicImports[0])!;
    const extra = chunks.find((chunk) => chunk.exports.includes('extraValue'))!;
    expect(second.dynamicImports).toEqual([destination.fileName]);
    expect(main.dynamicImports).toEqual([destination.fileName, extra.fileName]);
    for (const entry of entries) expect(entry.code).toContain(`./${destination.fileName}`);
    if (name === 'then') {
      expect(chunks).toHaveLength(5);
      expect(destination.imports).toEqual([target.fileName]);
      expect(destination.exports).not.toContain('then');
    } else {
      expect(chunks).toHaveLength(4);
      expect(destination).toBe(target);
    }
  } finally {
    await build.close();
  }
});

test.each([false, true])(
  'profilerNames preserves the deferred body name, minify %s',
  async (minify) => {
    const run = await bundle(
      {
        'main.mjs': "import defer * as ns from './target.mjs'; console.log(ns.frame);",
        'target.mjs': "import { caller } from './stack.mjs'; export const frame = caller();",
        'stack.mjs': `export function caller() {
        const previous = Error.prepareStackTrace;
        try {
          Error.prepareStackTrace = (_, frames) => frames[1].getFunctionName();
          return new Error().stack;
        } finally { Error.prepareStackTrace = previous; }
      }`,
      },
      { generatedCode: { profilerNames: true }, minify },
    );
    expect(run().map((name) => path.basename(name))).toEqual(['target.mjs']);
  },
);
