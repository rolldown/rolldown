import type { OutputChunk, Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// https://github.com/rolldown/rolldown/issues/9339
test('hash is stable when an unrelated isolated entry is added', async () => {
  const modules: Record<string, string> = {
    '/node_modules/react/index.js': 'exports.useState = function useState() {};',
    './src/a.js': 'import { useState } from "react"; console.log("a", useState);',
    './src/b.js': 'import { useState } from "react"; console.log("b", useState);',
    './src/c.js': 'import { useState } from "react"; console.log("c", useState);',
  };

  const virtualPlugin: Plugin = {
    name: 'virtual',
    resolveId(id) {
      if (id in modules) return id;
      if (id === 'react') return '/node_modules/react/index.js';
    },
    load(id) {
      return modules[id];
    },
  };

  async function build(input: string[]) {
    const bundle = await rolldown({ input, plugins: [virtualPlugin] });
    const out = await bundle.generate({
      entryFileNames: 'entries-[name]-[hash].js',
      chunkFileNames: 'chunk-[name]-[hash].js',
      codeSplitting: {
        groups: [{ name: 'react', test: /node_modules[\\/]react/ }],
      },
      format: 'esm',
    });
    await bundle.close();
    return new Map(
      out.output.filter((c): c is OutputChunk => c.type === 'chunk').map((c) => [c.name, c]),
    );
  }

  const two = await build(['./src/a.js', './src/b.js']);
  const three = await build(['./src/a.js', './src/b.js', './src/c.js']);

  for (const name of ['rolldown-runtime', 'react', 'a', 'b']) {
    expect(three.get(name)?.code).toBe(two.get(name)?.code);
    expect(three.get(name)?.fileName).toBe(two.get(name)?.fileName);
  }
});

// https://github.com/rolldown/rolldown/issues/5269
// `first` sets which entry finishes loading first. When it is `b.js`, its padding imports take
// module indices before `server.js`. The graph and its execution order stay the same.
async function buildWithLoadOrder(first: 'a.js' | 'b.js') {
  const modules: Record<string, string> = {
    'a.js':
      'import * as ns from "./server.js"; import { dt } from "./server.js"; console.log(ns, dt);',
    'b.js':
      'import "./pad0.js"; import "./pad1.js"; import "./pad2.js"; import { dt, v } from "./server.js"; console.log(dt, v);',
    'pad0.js': 'globalThis.pad0 = 0;',
    'pad1.js': 'globalThis.pad1 = 1;',
    'pad2.js': 'globalThis.pad2 = 2;',
    // The namespace of `server.js` is also named `server_exports`.
    'server.js': 'var server_exports = {}; export { server_exports as dt }; export const v = 1;',
  };
  const releases = new Map<string, () => void>();
  const bundle = await rolldown({
    input: ['a.js', 'b.js'],
    plugins: [
      {
        name: 'controlled-load-order',
        resolveId(id) {
          const file = id.replace('./', '');
          if (file in modules) return file;
        },
        async load(id) {
          if (id === 'a.js' || id === 'b.js') {
            await new Promise<void>((resolve) => {
              releases.set(id, resolve);
              if (releases.size === 2) releases.get(first)!();
            });
          }
          return modules[id];
        },
        moduleParsed(info) {
          // The first entry's imports have their module indices now, so let the other entry finish.
          if (info.id === (first === 'a.js' ? 'server.js' : 'pad0.js')) {
            releases.get(first === 'a.js' ? 'b.js' : 'a.js')!();
          }
        },
      },
    ],
  });
  const out = await bundle.generate({ format: 'esm' });
  await bundle.close();
  return out.output
    .filter((c): c is OutputChunk => c.type === 'chunk')
    .map((c) => [c.fileName, c.code]);
}

test('cross-chunk export names do not depend on module load order', async () => {
  expect(await buildWithLoadOrder('b.js')).toEqual(await buildWithLoadOrder('a.js'));
});

// With `preserveModules`, `a.js` and `b.js` render to the same code and get the same hashed file
// name, so one of them is renamed. `first` sets which entry finishes loading first, and so which of
// them gets the lower module index. The graph and its execution order stay the same.
async function buildPreservedModulesWithLoadOrder(first: 'e0.js' | 'e1.js') {
  const modules: Record<string, string> = {
    'e0.js': 'import { f } from "./a.js"; console.log(f(0));',
    'e1.js': 'import { f } from "./b.js"; console.log(f(1));',
    'a.js': 'export function f(x) { return x + 1; }',
    'b.js': 'export function f(x) { return x + 1; }',
  };
  const releases = new Map<string, () => void>();
  const bundle = await rolldown({
    input: ['e0.js', 'e1.js'],
    plugins: [
      {
        name: 'controlled-load-order',
        resolveId(id) {
          const file = id.replace('./', '');
          if (file in modules) return file;
        },
        async load(id) {
          if (id === 'e0.js' || id === 'e1.js') {
            await new Promise<void>((resolve) => {
              releases.set(id, resolve);
              if (releases.size === 2) releases.get(first)!();
            });
          }
          return modules[id];
        },
        moduleParsed(info) {
          // The first entry's import has its module index now, so let the other entry finish.
          if (info.id === (first === 'e0.js' ? 'a.js' : 'b.js')) {
            releases.get(first === 'e0.js' ? 'e1.js' : 'e0.js')!();
          }
        },
      },
    ],
  });
  const out = await bundle.generate({
    preserveModules: true,
    minify: true,
    entryFileNames: '[hash].js',
  });
  await bundle.close();
  return out.output
    .filter((c): c is OutputChunk => c.type === 'chunk')
    .map((c) => [c.fileName, c.code]);
}

test('preserved modules with the same code are named independently of module load order', async () => {
  expect(await buildPreservedModulesWithLoadOrder('e1.js')).toEqual(
    await buildPreservedModulesWithLoadOrder('e0.js'),
  );
});
