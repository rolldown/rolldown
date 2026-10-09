import { spawnSync } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import type { InputOptions, OutputChunk, OutputOptions } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const modules = {
  './entry.js': `
    export async function run() {
      const bar = require('./req.js').bar;
      const { Foo } = await import('./lib.js');
      return { bar: bar.name, Foo };
    }
  `,
  './req.js': "export { bar } from './barrel.js';",
  './lib.js': "export { Foo } from './barrel.js';",
  './barrel.js': "export { Foo } from './foo.js'; export { bar } from './bar.js';",
  './foo.js': "export const Foo = { name: 'Foo' };",
  './bar.js': "export const bar = { name: 'bar' };",
};

type Mode = 'wrap-all' | 'on-demand';
interface Case {
  modules?: Record<string, string>;
  input?: InputOptions;
  output?: OutputOptions;
  loadsFoo: boolean;
}

function execute(entry: string): unknown {
  const result = spawnSync(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      `
        import { createRequire } from 'node:module';
        import { pathToFileURL } from 'node:url';
        const url = pathToFileURL(process.argv[1]);
        globalThis.require = createRequire(url);
        globalThis.events = [];
        try {
          const namespace = await import(url);
          if (typeof namespace.run !== 'function' && !('result' in globalThis)) {
            throw new Error('missing test result');
          }
          const value = typeof namespace.run === 'function' ? await namespace.run() : await globalThis.result;
          console.log(JSON.stringify({ value, events: globalThis.events }));
        } catch (error) {
          console.log(JSON.stringify({ error: { name: error.name, message: error.message }, events: globalThis.events }));
        }
      `,
      entry,
    ],
    { encoding: 'utf8', timeout: 5000 },
  );
  expect(result.status, result.stderr).toBe(0);
  return JSON.parse(result.stdout.trim());
}

function staticallyLoadedModules(chunks: OutputChunk[]): string[] {
  const byFile = new Map(chunks.map((chunk) => [chunk.fileName, chunk]));
  const pending = [chunks.find((chunk) => chunk.isEntry)!.fileName];
  const visited = new Set<string>();
  const loaded = new Set<string>();
  for (const file of pending) {
    if (visited.has(file)) continue;
    visited.add(file);
    const chunk = byFile.get(file);
    if (!chunk) continue;
    for (const id of chunk.moduleIds) loaded.add(id);
    pending.push(...chunk.imports);
  }
  return [...loaded];
}

async function check(mode: Mode, options: Case): Promise<void> {
  const root = path.join(import.meta.dirname, 'wrapped-esm-reexport-initialization/dist');
  await fs.mkdir(root, { recursive: true });
  const directory = await fs.mkdtemp(path.join(root, 'wrapped-esm-'));
  const sources = { ...modules, ...options.modules };
  try {
    for (const subdirectory of ['source', 'output']) {
      const dir = path.join(directory, subdirectory);
      await fs.mkdir(dir);
      await fs.writeFile(path.join(dir, 'package.json'), '{"type":"module"}');
      for (const [id, code] of Object.entries(sources)) {
        await fs.writeFile(path.join(dir, id), code);
      }
    }
    const expected = execute(path.join(directory, 'source/entry.js'));
    expect(expected).not.toHaveProperty('error');
    const bundle = await rolldown({
      input: './entry.js',
      ...options.input,
      experimental: {
        ...options.input?.experimental,
        onDemandWrapping: mode === 'on-demand',
      },
      plugins: [
        {
          name: 'virtual',
          resolveId: (id) => (id in sources ? id : undefined),
          load: (id) => sources[id as keyof typeof sources],
        },
      ],
    });
    try {
      const { output } = await bundle.write({
        dir: path.join(directory, 'output'),
        format: 'esm',
        entryFileNames: '[name].mjs',
        chunkFileNames: '[name]-[hash].mjs',
        strictExecutionOrder: true,
        ...options.output,
      });
      const chunks = output.filter((output): output is OutputChunk => output.type === 'chunk');
      const entry = chunks.find((chunk) => chunk.isEntry)!;
      const actual = execute(path.join(directory, 'output', entry.fileName));
      expect(actual).toEqual(expected);
      expect(staticallyLoadedModules(chunks).some((id) => id.endsWith('/foo.js'))).toBe(
        options.loadsFoo,
      );
    } finally {
      await bundle.close();
    }
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
}

test.each<Mode>(['wrap-all', 'on-demand'])(
  'routes independent ESM re-exports to their consumers (%s)',
  async (mode) => {
    for (const output of [
      {},
      { minify: true },
      { format: 'cjs', entryFileNames: '[name].cjs', chunkFileNames: '[name]-[hash].cjs' },
      {
        codeSplitting: {
          includeDependenciesRecursively: false,
          groups: [{ name: 'leaf', test: /foo\.js$/ }],
        },
      },
    ] satisfies OutputOptions[]) {
      await check(mode, {
        input: output.codeSplitting ? { preserveEntrySignatures: 'allow-extension' } : undefined,
        output,
        loadsFoo: false,
      });
    }
    if (mode === 'wrap-all') {
      await check(mode, {
        input: { preserveEntrySignatures: false },
        output: { codeSplitting: { experimentalInlineCommonChunks: { maxSize: 10 * 1024 } } },
        modules: {
          './entry.js':
            "const bar = require('./req.js').bar; globalThis.result = import('./lib.js').then(({ Foo }) => ({ bar: bar.name, Foo }));",
        },
        loadsFoo: false,
      });
    }
    await check(mode, {
      input: { experimental: { chunkOptimization: false } },
      modules: {
        './barrel.js': "export { Foo } from './forward.js'; export { bar } from './bar.js';",
        './forward.js': "export * from './foo.js';",
      },
      loadsFoo: false,
    });
  },
);

test.each<Mode>(['wrap-all', 'on-demand'])(
  'initializes opaque namespace exports through wrapped forwarders (%s)',
  async (mode) => {
    const options: Case = {
      modules: {
        './barrel.js': "export * as Reorder from './namespace.js'; export { bar } from './bar.js';",
        './namespace.js': "export { Foo } from './foo.js'; export { Item } from './item.js';",
        './item.js': "export const Item = { name: 'Item' };",
        './lib.js': `
          import * as api from './barrel.js';
          export * from './barrel.js';
          export const value = api.bar;
        `,
        './entry.js': `
          export async function run() {
            require('./req.js');
            const { Reorder } = await import('./lib.js');
            return Object.keys(Reorder).map((key) => Reorder[key].name);
          }
        `,
      },
      loadsFoo: false,
    };
    await check(mode, options);
    await check(mode, {
      ...options,
      modules: {
        ...options.modules,
        './react.js': options.modules!['./lib.js'],
        './lib.js': `
          import { Reorder } from './react.js';
          export const check = () => Object.keys(Reorder).map((key) => Reorder[key].name);
        `,
        './entry.js': `
          export async function run() {
            require('./req.js');
            return (await import('./lib.js')).check();
          }
        `,
      },
    });
  },
);

test.each<Mode>(['wrap-all', 'on-demand'])(
  'initializes re-exported thenables before promise resolution (%s)',
  async (mode) => {
    await check(mode, {
      modules: {
        './foo.js':
          "export const Foo = { name: 'Foo' }; export function then(resolve) { resolve({ Foo }); }",
        './barrel.js': "export { Foo, then } from './foo.js'; export { bar } from './bar.js';",
        './lib.js': "export { Foo, then } from './barrel.js';",
      },
      loadsFoo: false,
    });
  },
);

test.each<Mode>(['wrap-all', 'on-demand'])(
  'keeps object identity, live exports and deferred function reads (%s)',
  async (mode) => {
    await check(mode, {
      modules: {
        './foo.js':
          "export let Foo = { name: 'Foo' }; export function update() { Foo = { name: 'changed' }; }",
        './barrel.js': "export { Foo, update } from './foo.js'; export { bar } from './bar.js';",
        './lib.js': "export { Foo, update } from './barrel.js';",
        './entry.js': `
          export async function run() {
            require('./req.js');
            const one = await import('./lib.js');
            const two = await import('./lib.js');
            const original = one.Foo;
            const same = original === two.Foo;
            one.update();
            return { same, original: original.name, updated: two.Foo.name };
          }
        `,
      },
      loadsFoo: false,
    });
    await check(mode, {
      modules: {
        './foo.js':
          "export let Foo = { name: 'Foo' }; export function update() { Foo = { name: 'changed' }; }",
        './bar.js': `
          import { update } from './foo.js';
          export function bar() { update(); return 'bar'; }
        `,
        './entry.js': `
          export async function run() {
            const bar = require('./req.js').bar();
            return { bar, name: (await import('./lib.js')).Foo.name };
          }
        `,
      },
      loadsFoo: true,
    });
    await check(mode, {
      modules: {
        './state.js': "export let value = 'before'; export function mutate() { value = 'after'; }",
        './foo.js': "import { value } from './state.js'; export function Foo() { return value; }",
        './entry.js': `
          import { mutate } from './state.js';
          export async function run() {
            require('./req.js');
            mutate();
            return (await import('./lib.js')).Foo();
          }
        `,
      },
      loadsFoo: false,
    });
    await check(mode, {
      modules: {
        './foo.js':
          "const name = 'Foo'; const data = { name }; export const Foo = { name, data, get phase() { return globalThis.phase; } };",
        './entry.js': `
          export async function run() {
            require('./req.js');
            globalThis.phase = 'after';
            const { Foo } = await import('./lib.js');
            return { same: Foo.data.name === Foo.name, phase: Foo.phase };
          }
        `,
      },
      loadsFoo: false,
    });
    await check(mode, {
      modules: {
        './state.js': "export let value = 'before'; export function mutate() { value = 'after'; }",
        './foo.js': "import { value } from './state.js'; export class Foo { snapshot = value; }",
        './entry.js': `
          import { mutate } from './state.js';
          export async function run() {
            require('./req.js');
            mutate();
            const { Foo } = await import('./lib.js');
            return new Foo().snapshot;
          }
        `,
      },
      loadsFoo: false,
    });
  },
);

const timingCases: Record<string, Case> = {
  'mutable imported snapshots': {
    modules: {
      './state.js': "export let value = 'before'; export function mutate() { value = 'after'; }",
      './foo.js': "import { value } from './state.js'; export const Foo = { snapshot: value };",
      './entry.js': `
        import { mutate } from './state.js';
        export async function run() {
          require('./req.js');
          mutate();
          return (await import('./lib.js')).Foo;
        }
      `,
    },
    loadsFoo: true,
  },
  'annotated pure calls': {
    modules: {
      './foo.js':
        "export const Foo = /* @__PURE__ */ (() => ({ snapshot: globalThis.phase ?? 'before' }))();",
      './entry.js': `
        export async function run() {
          require('./req.js');
          globalThis.phase = 'after';
          return (await import('./lib.js')).Foo;
        }
      `,
    },
    loadsFoo: true,
  },
  'getters under propertyReadSideEffects: false': {
    input: { treeshake: { propertyReadSideEffects: false, moduleSideEffects: false } },
    modules: {
      './foo.js':
        "const state = { get value() { return globalThis.phase ?? 'before'; } }; export const Foo = { snapshot: state.value };",
      './entry.js': `
        export async function run() {
          require('./req.js');
          globalThis.phase = 'after';
          return (await import('./lib.js')).Foo;
        }
      `,
    },
    loadsFoo: true,
  },
  'effectful dependencies': {
    modules: { './bar.js': "globalThis.events.push('bar'); export const bar = { name: 'bar' };" },
    loadsFoo: true,
  },
  'eager calls to hoisted functions': {
    modules: {
      './foo.js': "export const Foo = { name: 'Foo' }; export function read() { return Foo; }",
      './bar.js': "import { read } from './foo.js'; export const bar = /* @__PURE__ */ read();",
    },
    loadsFoo: true,
  },
  'object spread getters': {
    input: { treeshake: { moduleSideEffects: false } },
    modules: {
      './foo.js':
        "const state = { get snapshot() { return globalThis.phase ?? 'before'; } }; export const Foo = { ...state };",
      './entry.js': `
        export async function run() {
          require('./req.js');
          globalThis.phase = 'after';
          return (await import('./lib.js')).Foo;
        }
      `,
    },
    loadsFoo: true,
  },
  'destructuring getters': {
    input: { treeshake: { moduleSideEffects: false } },
    modules: {
      './foo.js':
        "const { snapshot } = { get snapshot() { return globalThis.phase ?? 'before'; } }; export const Foo = { snapshot };",
      './entry.js': `
        export async function run() {
          require('./req.js');
          globalThis.phase = 'after';
          return (await import('./lib.js')).Foo;
        }
      `,
    },
    loadsFoo: true,
  },
  'object coercions': {
    input: { treeshake: { moduleSideEffects: false } },
    modules: {
      './foo.js':
        'const state = { valueOf() { return globalThis.phase ?? 1; } }; export const Foo = { snapshot: +state };',
      './entry.js': `
        export async function run() {
          require('./req.js');
          globalThis.phase = 2;
          return (await import('./lib.js')).Foo;
        }
      `,
    },
    loadsFoo: true,
  },
  'class definition-time reads': {
    modules: {
      './foo.js': "export class Foo { static snapshot = globalThis.phase ?? 'before'; }",
      './entry.js': `
        export async function run() {
          require('./req.js');
          globalThis.phase = 'after';
          return (await import('./lib.js')).Foo.snapshot;
        }
      `,
    },
    loadsFoo: true,
  },
  'throwing primitive coercions': {
    input: { treeshake: { moduleSideEffects: false } },
    modules: {
      './foo.js': 'export const Foo = +1n;',
      './entry.js': `
        export const load = () => import('./lib.js');
        export async function run() {
          try { require('./req.js'); } catch (error) { return error.name; }
          return 'no error';
        }
      `,
    },
    loadsFoo: true,
  },
  'static external dependencies': {
    input: { external: ['./effect.js'], makeAbsoluteExternalsRelative: false },
    modules: {
      './foo.js': "import './effect.js'; export const Foo = { name: 'Foo' };",
      './effect.js': "globalThis.events.push('external');",
    },
    loadsFoo: true,
  },
  'synchronous re-export cycles': {
    modules: {
      './foo.js':
        "import { bar } from './barrel.js'; export const Foo = { name: 'Foo', read: () => bar.name };",
    },
    loadsFoo: true,
  },
  'required complete namespaces': {
    modules: {
      './entry.js':
        "export async function run() { const namespace = require('./barrel.js'); await import('./lib.js'); return { keys: Object.keys(namespace), Foo: namespace.Foo }; }",
    },
    loadsFoo: true,
  },
  'disabled tree shaking': { input: { treeshake: false }, loadsFoo: true },
  'explicit effectful module contracts': {
    input: { treeshake: { moduleSideEffects: () => true } },
    loadsFoo: true,
  },
  'function names': {
    output: { keepNames: true },
    modules: {
      './foo.js': 'export function Foo() {}',
      './entry.js': `
        export async function run() {
          require('./req.js');
          return (await import('./lib.js')).Foo.name;
        }
      `,
    },
    loadsFoo: true,
  },
  'cached initialization errors': {
    modules: {
      './foo.js':
        "export const Foo = /* @__PURE__ */ (() => { globalThis.events.push('foo'); throw new Error('initialization failed'); })();",
      './bar.js': "globalThis.events.push('bar'); export const bar = { name: 'bar' };",
      './entry.js': `
        export const load = () => import('./lib.js');
        export async function run() {
          const errors = [];
          for (let attempt = 0; attempt < 2; attempt++) {
            try { require('./req.js'); } catch (error) { errors.push(error); }
          }
          return { same: errors[0] === errors[1], messages: errors.map(error => error.message) };
        }
      `,
    },
    loadsFoo: true,
  },
};

test.each<Mode>(['wrap-all', 'on-demand'])(
  'preserves unproven initializers beside independent forwarding routes (%s)',
  async (mode) => {
    for (const name of ['annotated pure calls', 'mutable imported snapshots']) {
      const options = timingCases[name];
      for (const forward of ["export { bar } from './bar.js';", "export * from './bar.js';"]) {
        await check(mode, {
          ...options,
          modules: {
            ...options.modules,
            './barrel.js': "export { Foo } from './foo.js'; export { bar } from './inner.js';",
            './inner.js': forward,
          },
          loadsFoo: true,
        });
      }
    }
  },
);

for (const [name, options] of Object.entries(timingCases)) {
  test.each<Mode>(['wrap-all', 'on-demand'])(`preserves ${name} (%s)`, async (mode) => {
    await check(mode, options);
  });
}
