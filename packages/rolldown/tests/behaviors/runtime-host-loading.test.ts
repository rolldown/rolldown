import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// Compare source execution with bundled output in fresh processes. A comparison of two bundled
// configurations can miss a runtime-placement bug shared by both configurations.
// RUNTIME_HOST_FUZZ_SEEDS=<n> expands the corpus; each seed also selects an output configuration.
const seedCount = Number(process.env.RUNTIME_HOST_FUZZ_SEEDS ?? 18);

function generateGraph(seed: number): Record<string, string> {
  let state = seed >>> 0;
  const random = (limit: number) => {
    state = (state + 0x6d2b79f5) >>> 0;
    let value = Math.imul(state ^ (state >>> 15), state | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return Math.floor((((value ^ (value >>> 14)) >>> 0) / 4294967296) * limit);
  };
  const count = 2 + random(5);
  // Seed 1 must have no link-time helper demand in the logger; it detects the original bug.
  const commonJs = random(3) === 0 && seed !== 1;
  // Full namespace reads create helper demand at link time. Keep most imports as named reads.
  const checkIdentity = seed % 5 === 0;
  const files: Record<string, string> = {
    'logger.js': `
      ${commonJs ? "import legacy from './legacy.cjs';" : ''}
      let calls = 0;
      export function log(value) {
        return [value, ++calls, ${commonJs ? 'legacy.value' : "'esm'"}];
      }
    `,
    'legacy.cjs': `globalThis.events.push('legacy'); module.exports = { value: 'cjs' };`,
    'namespace.js': `
      globalThis.events.push('namespace');
      export let value = 0;
      export function bump() { value++; }
    `,
    'a.js': `
      import { log } from './logger.js';
      globalThis.events.push('a');
      export function run() { return log('a'); }
    `,
    'b.js': `
      globalThis.events.push('b');
      export async function run() {
        const { log } = await import('./logger.js');
        ${
          checkIdentity
            ? `const first = await import('./logger.js'); const second = await import('./logger.js');`
            : ''
        }
        const route = await import('./route0.js');
        return [${checkIdentity ? 'first === second,' : ''} log('b'), await route.run(2)];
      }
    `,
    // This import must not contribute an importer to the already-loaded analysis.
    'dead.js': `
      globalThis.events.push('dead');
      export function unused() { return import('./logger.js'); }
    `,
  };
  for (const name of ['c', 'd']) {
    files[`${name}.js`] = `
      import * as ns from './namespace.js';
      globalThis.events.push('${name}');
      export async function run() {
        ns.bump();
        const result = [Object.keys(ns).sort(), ns.value];
        ${random(2) ? `result.push(await (await import('./route${random(count)}.js')).run(2));` : ''}
        return result;
      }
    `;
  }
  for (let i = 0; i < count; i++) {
    const target = random(count);
    const staticLogger = random(2) === 0;
    files[`route${i}.js`] = `
      ${staticLogger ? "import { log } from './logger.js';" : ''}
      ${i === 0 ? "import { unused } from './dead.js';" : ''}
      globalThis.events.push('route${i}');
      export async function run(depth) {
        ${staticLogger ? '' : "const { log } = await import('./logger.js');"}
        const result = [log('route${i}')];
        if (depth > 0) result.push(await (await import('./route${target}.js')).run(depth - 1));
        return result;
      }
    `;
  }
  // Some dynamic targets are re-export facades. Others form cycles or have several importers.
  if (seed % 2 === 0) {
    files['forward.js'] = "export * from './logger.js';";
    files['b.js'] = files['b.js'].replaceAll("'./logger.js'", "'./forward.js'");
  }
  // ESM-only cases also cover asynchronous evaluation before a dynamic import can start.
  if (seed % 4 === 0 && (seed - 1) % 6 < 3) {
    files['namespace.js'] = `await Promise.resolve();\n${files['namespace.js']}`;
  }
  return files;
}

const runner = `
  import { createRequire } from 'node:module';
  import { pathToFileURL } from 'node:url';
  const require = createRequire(import.meta.url);
  globalThis.events = [];
  const results = [];
  for (const file of JSON.parse(process.argv[1])) {
    const ns = file.endsWith('.cjs') ? require(file) : await import(pathToFileURL(file));
    const loaded = [...globalThis.events];
    const first = await ns.run();
    const second = await ns.run();
    results.push({ keys: Object.keys(ns).sort(), loaded, first, second, events: [...globalThis.events] });
  }
  console.log(JSON.stringify(results));
`;

function execute(files: string[]): unknown {
  return JSON.parse(
    execFileSync(process.execPath, ['--input-type=module', '-e', runner, JSON.stringify(files)], {
      encoding: 'utf8',
      timeout: 10_000,
    }),
  );
}

test.each(Array.from({ length: seedCount }, (_, i) => i + 1))(
  'runtime hosting preserves source execution for seed %i',
  async (seed) => {
    const mode = (seed - 1) % 6;
    const format = mode < 3 ? 'esm' : 'cjs';
    const strictExecutionOrder = mode % 3 !== 0;
    const extension = format === 'esm' ? 'mjs' : 'cjs';
    const preserveModules = seed % 13 === 0;
    const directory = await mkdtemp(path.join(tmpdir(), 'rolldown-runtime-host-'));
    try {
      const source = path.join(directory, 'source');
      const output = path.join(directory, 'output');
      await mkdir(source);
      await writeFile(path.join(directory, 'package.json'), '{"type":"module"}');
      const files = generateGraph(seed);
      files['emitted.js'] = "export { run } from './b.js';";
      await Promise.all(
        Object.entries(files).map(([name, code]) => writeFile(path.join(source, name), code)),
      );
      const input = Object.fromEntries(['a', 'b', 'c', 'd'].map((name) => [name, `${name}.js`]));
      if (seed % 11 === 0) input.alias = 'a.js';
      const bundle = await rolldown({
        cwd: source,
        input,
        preserveEntrySignatures: seed % 2 === 0 ? 'strict' : 'exports-only',
        experimental: { onDemandWrapping: mode % 3 === 2 },
        plugins: [
          {
            name: 'emitted-runtime-consumer',
            buildStart() {
              if (seed % 7 === 0) {
                this.emitFile({
                  type: 'chunk',
                  id: './emitted.js',
                  fileName: `emitted.${extension}`,
                });
              }
            },
          },
        ],
      });
      const preservedEntries = new Map<string, string>();
      try {
        const generated = await bundle.write({
          dir: output,
          format,
          strictExecutionOrder,
          minify: Math.floor((seed - 1) / 6) % 2 === 1,
          entryFileNames: `[name].${extension}`,
          chunkFileNames: `[name]-[hash].${extension}`,
          // Grouping can move unrelated modules together. Strict mode must preserve their effects.
          ...(seed % 5 === 0 && strictExecutionOrder
            ? { codeSplitting: { groups: [{ name: 'group', test: /(?:logger|namespace)\.js$/ }] } }
            : {}),
          preserveModules,
        });
        for (const chunk of generated.output) {
          if (chunk.type === 'chunk' && chunk.isEntry && chunk.facadeModuleId) {
            preservedEntries.set(path.basename(chunk.facadeModuleId), chunk.fileName);
          }
        }
      } finally {
        await bundle.close();
      }
      const entries = Object.keys(input);
      if (seed % 7 === 0) {
        input.emitted = 'emitted.js';
        entries.push('emitted');
      }
      const scenarios = [...entries.map((entry) => [entry]), entries, [...entries].reverse()];
      for (const scenario of scenarios) {
        expect(
          execute(
            scenario.map((name) => {
              // preserveModules emits one file per module, including for duplicate input aliases.
              const file = preserveModules
                ? preservedEntries.get(input[name])!
                : `${name}.${extension}`;
              return path.join(output, file);
            }),
          ),
          `seed ${seed}, ${format}, mode ${mode}, entries ${scenario.join(' then ')}`,
        ).toEqual(execute(scenario.map((name) => path.join(source, input[name]))));
      }
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  },
  30_000,
);
