import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import type { InputOptions, OutputChunk, OutputOptions } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// A fold moves the runtime into a chunk with user code. Then the runtime starts when that chunk
// starts, not before the first effect of each consumer. User code cannot see this change only for
// the runtime that rolldown generates: it declares helpers, and its only import is the built-in
// `node:module`. Each case below makes the start of the runtime visible. The expected logs come from
// the layout that keeps the runtime in its own chunk. Each consumer imports that chunk before its
// other imports, so the runtime starts before the first effect of the entry.
const runtimeId = '\0rolldown/runtime.js';

interface Options {
  input?: Omit<InputOptions, 'cwd' | 'input'>;
  output?: OutputOptions;
}

// Writes the files, bundles the entries, and runs each entry in a new process.
async function bundle(
  files: Record<string, string>,
  entries: string[],
  options: (source: string) => Options,
): Promise<{ chunks: OutputChunk[]; logs: Record<string, string[]> }> {
  const directory = await mkdtemp(path.join(tmpdir(), 'rolldown-known-runtime-'));
  try {
    const source = path.join(directory, 'source');
    const output = path.join(directory, 'output');
    await mkdir(source);
    await Promise.all(
      Object.entries(files).map(([name, code]) => writeFile(path.join(source, name), code)),
    );
    const { input, output: outputOptions } = options(source);
    const build = await rolldown({
      cwd: source,
      input: Object.fromEntries(entries.map((name) => [name, `${name}.js`])),
      ...input,
    });
    let chunks: OutputChunk[];
    try {
      const generated = await build.write({
        dir: output,
        format: 'esm',
        entryFileNames: '[name].mjs',
        chunkFileNames: '[name]-[hash].mjs',
        ...outputOptions,
      });
      chunks = generated.output.filter((file): file is OutputChunk => file.type === 'chunk');
    } finally {
      await build.close();
    }
    const logs: Record<string, string[]> = {};
    for (const entry of entries) {
      const run = execFileSync(process.execPath, [path.join(output, `${entry}.mjs`)], {
        encoding: 'utf8',
        timeout: 10_000,
      });
      logs[entry] = run.trim().split('\n');
    }
    return { chunks, logs };
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

// Each group chunk has an effect, and its entry imports it before `shared.js`.
const groups: OutputOptions['codeSplitting'] = {
  groups: [
    { name: 'p1', test: /p1\.js$/ },
    { name: 'p2', test: /p2\.js$/ },
  ],
};

function edit(code: string, search: string | RegExp, replacement: string): string {
  const next = code.replace(search, replacement);
  // Fail when the runtime source changes. Without the edit, the case tests nothing.
  if (next === code) throw new Error(`The runtime source has no match for ${search}.`);
  return next;
}

// A plugin can transform the runtime module. Then the runtime can read an imported binding at its
// start, or wait for an import with top-level await.
const transforms = {
  // The runtime reads a mutable binding at its start, and `__exportAll` logs that value.
  // The group chunks change the binding.
  reads: {
    external: 'export let value = 0;\nexport function bump() { value++; }',
    group: (n: number) => `import { bump } from './external.mjs';\nbump();\nconsole.log('P${n}');`,
    transform: (code: string) =>
      "import { value as __value } from './external.mjs';\nvar __captured = __value;\n" +
      edit(
        code,
        /^export var __exportAll = \(.*\) => \{$/m,
        "$&\n  console.log('captured', __captured);",
      ),
  },
  // The runtime imports a module with top-level await. A microtask of each group chunk shows the
  // order of evaluation.
  waits: {
    external: 'await 0;\nexport const define = Object.defineProperty;',
    group: (n: number) => `console.log('P${n}');\nqueueMicrotask(() => console.log('tick${n}'));`,
    transform: (code: string) =>
      "import { define as __define } from './external.mjs';\n" +
      edit(
        code,
        'export var __defProp = Object.defineProperty;',
        'export var __defProp = __define;',
      ),
  },
};

const transformScenarios: {
  name: string;
  kind: keyof typeof transforms;
  logs: Record<string, string[]>;
}[] = [
  // `shared.js` is a common chunk. Each entry imports it after its group chunk.
  {
    name: 'reads an import, two consumers',
    kind: 'reads',
    logs: { c1: ['P1', 'H', 'captured 0', 'C1'], c2: ['P2', 'H', 'captured 0', 'C2'] },
  },
  {
    name: 'waits for an import, two consumers',
    kind: 'waits',
    logs: { c1: ['P1', 'H', 'tick1', 'C1'], c2: ['P2', 'H', 'tick2', 'C2'] },
  },
  // `shared.js` is in the chunk of `c1`, so `H` waits for the runtime.
  {
    name: 'reads an import, one consumer',
    kind: 'reads',
    logs: { c1: ['P1', 'H', 'captured 0', 'C1'] },
  },
  {
    name: 'waits for an import, one consumer',
    kind: 'waits',
    logs: { c1: ['P1', 'tick1', 'H', 'C1'] },
  },
];

test.each(transformScenarios)('a transformed runtime starts first: $name', async (scenario) => {
  const { external, group, transform } = transforms[scenario.kind];
  const files: Record<string, string> = {
    'external.mjs': external,
    'shared.js': "console.log('H');",
    'c3.js': "console.log('C3');",
  };
  for (const n of [1, 2]) {
    files[`p${n}.js`] = group(n);
    files[`d${n}.js`] = `export const value = ${n};`;
    // The namespace object of `d${n}.js` needs `__exportAll`, so this entry is a consumer.
    files[`c${n}.js`] = `
      import './p${n}.js';
      import './shared.js';
      import * as ns from './d${n}.js';
      globalThis.ns${n} = ns;
      console.log('C${n}');
    `;
  }
  const { chunks, logs } = await bundle(files, [...Object.keys(scenario.logs), 'c3'], (source) => ({
    input: {
      plugins: [
        {
          name: 'transform-runtime',
          resolveId(id) {
            if (id === './external.mjs') {
              const url = pathToFileURL(path.join(source, 'external.mjs')).href;
              return { id: url, external: true, moduleSideEffects: false };
            }
          },
          transform(code, id) {
            if (id === runtimeId) return transform(code);
          },
        },
      ],
    },
    output: { codeSplitting: groups },
  }));
  // `c3.js` uses no helper. Each entry imports a runtime with side effects, so an empty list
  // shows that this runtime has none. A placement that checks only side effects reaches this case.
  expect(chunks.find((chunk) => chunk.name === 'c3')?.imports).toEqual([]);
  for (const [entry, expected] of Object.entries(scenario.logs)) {
    expect(logs[entry], entry).toEqual(expected);
  }
});

// The runtime of Node ESM output imports `createRequire` from `node:module`. A `resolveId` hook and
// `output.paths` can each send that import to another module. The shim below logs when it starts,
// and it has `moduleSideEffects: false`, so only the check of the import itself finds it.
const redirects: {
  name: string;
  options: (shim: string) => Options;
}[] = [
  {
    name: 'resolveId',
    options: (shim) => ({
      input: {
        plugins: [
          {
            name: 'redirect-node-module',
            resolveId(id, importer) {
              if (id === 'node:module' && importer === runtimeId) {
                return { id: shim, external: true, moduleSideEffects: false };
              }
            },
          },
        ],
      },
    }),
  },
  {
    // The import keeps the resolved ID `node:module`. Only the emitted path changes.
    name: 'output.paths',
    options: (shim) => ({ output: { paths: { 'node:module': shim } } }),
  },
  {
    // A `paths` function runs only after the placement, so the placement cannot read its result.
    name: 'output.paths function',
    options: (shim) => ({ output: { paths: (id) => (id === 'node:module' ? shim : id) } }),
  },
];

test.each(redirects)('a redirected node:module import starts first: $name', async (redirect) => {
  const files: Record<string, string> = {
    'shim.mjs': "export { createRequire } from 'node:module';\nconsole.log('shim effect');",
    // `require` needs `__require`, so the runtime imports `node:module`.
    'shared.js': "console.log('H');\nexport const h = () => require('node:path');",
    'c3.js': "console.log('C3');",
  };
  for (const n of [1, 2]) {
    files[`p${n}.js`] = `console.log('P${n}');`;
    files[`d${n}.js`] = `export const value = ${n};`;
    files[`c${n}.js`] = `
      import './p${n}.js';
      import { h } from './shared.js';
      import * as ns from './d${n}.js';
      globalThis.h${n} = h;
      globalThis.ns${n} = ns;
      console.log('C${n}');
    `;
  }
  const { chunks, logs } = await bundle(files, ['c1', 'c2', 'c3'], (source) => {
    const { input, output } = redirect.options(pathToFileURL(path.join(source, 'shim.mjs')).href);
    return {
      input: { platform: 'node', ...input },
      output: { codeSplitting: groups, ...output },
    };
  });
  expect(chunks.find((chunk) => chunk.name === 'c3')?.imports).toEqual([]);
  expect(logs.c1).toEqual(['shim effect', 'P1', 'H', 'C1']);
  expect(logs.c2).toEqual(['shim effect', 'P2', 'H', 'C2']);
});

// The checks above must not stop the fold of the runtime that rolldown generates for Node ESM output.
test('the generated Node ESM runtime moves into its only consumer', async () => {
  const files = {
    'c1.js': "const fs = require('node:fs');\nconsole.log('C1', typeof fs.readFileSync);",
    'c3.js': "console.log('C3');",
  };
  const { chunks, logs } = await bundle(files, ['c1', 'c3'], () => ({
    input: { platform: 'node' },
  }));
  const host = chunks.find((chunk) => chunk.moduleIds.includes(runtimeId));
  expect(host?.name).toBe('c1');
  expect(host?.code).toMatch(/from ["']node:module["']/);
  expect(chunks.find((chunk) => chunk.name === 'c3')?.imports).toEqual([]);
  expect(logs).toEqual({ c1: ['C1 function'], c3: ['C3'] });
});
