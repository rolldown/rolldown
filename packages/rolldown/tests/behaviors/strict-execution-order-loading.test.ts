import { execFileSync } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import type { OutputChunk } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

const prefix = '\0loading/';
const sources: Record<string, string> = {
  'main.js': `
    import { ready } from './barrel.js';
    globalThis.startupValue = ready();
    globalThis.events.push('main');
    globalThis.loadProduct = () => import('./product.js').then(m => m.product());
    globalThis.loadLegacy = () => import('./legacy.js').then(m => m.loadLegacy());
  `,
  'barrel.js': `
    export { ready } from './used.js';
    export { extra } from './helper.js';
    globalThis.events.push('barrel');
  `,
  'used.js': `
    globalThis.events.push('used');
    export const ready = () => 'ready';
  `,
  'helper.js': `
    import { state } from './data.js';
    export const extra = value => value ?? state.DEFAULT;
  `,
  'data.js': `export const state = { DEFAULT: 'fallback' };`,
  'product.js': `
    import { extra } from './barrel.js';
    export function product() {
      globalThis.productCalls = (globalThis.productCalls ?? 0) + 1;
      return extra(null);
    }
  `,
  'registry.js': `
    export function loadData() { return import('./data.js').then(m => m.state); }
  `,
  'legacy.js': `
    import { loadData } from './registry.js';
    export function loadLegacy() { return loadData(); }
  `,
};

interface Scenario {
  name: string;
  modules?: Record<string, string>;
  initialValue?: string;
  productValue?: string;
  events?: string[];
  unloaded?: string[];
}

const scenarios: Scenario[] = [
  { name: 'direct re-exports' },
  {
    name: 'a lazy require reaches a dynamic data entry',
    modules: {
      'legacy.js': `export function loadLegacy() { return require('./registry.js').loadData(); }`,
    },
  },
  {
    name: 'an import and export list',
    modules: {
      'barrel.js': `
        import { ready } from './used.js';
        import { extra } from './helper.js';
        export { ready, extra };
        globalThis.events.push('barrel');
      `,
    },
  },
  {
    name: 'a pure barrel with an effectful dependency',
    modules: {
      'barrel.js': `export { ready } from './used.js'; export { extra } from './helper.js';`,
    },
    events: ['used', 'main'],
  },
  {
    name: 'a pure enum factory',
    modules: {
      'data.js': `
        export let state = /* @__PURE__ */ function(state) {
          state['DEFAULT'] = 'fallback';
          return state;
        }({});
      `,
    },
  },
  {
    name: 'a snapshot precedes an imported mutation',
    modules: {
      'state.js': `export let value = 'before'; export function mutate() { value = 'after'; }`,
      'data.js': `
        import { value } from './state.js';
        export const state = { DEFAULT: /* @__PURE__ */ (() => value)() };
      `,
      'mutation.js': `
        import { mutate } from './state.js';
        mutate();
        globalThis.events.push('mutation');
      `,
      'main.js': sources['main.js'].replace(
        "import { ready } from './barrel.js';",
        "import { ready } from './barrel.js'; import './mutation.js';",
      ),
    },
    productValue: 'before',
    events: ['used', 'barrel', 'mutation', 'main'],
  },
  {
    name: 'a synchronous ESM cycle',
    modules: {
      'data.js': `
        import { ready } from './used.js';
        export const state = { DEFAULT: 'fallback', read: () => ready() };
      `,
      'used.js': `
        import { state } from './data.js';
        globalThis.events.push('used');
        export const ready = () => state.DEFAULT;
      `,
    },
    initialValue: 'fallback',
  },
  {
    name: 'an opaque namespace consumer',
    modules: {
      'main.js': sources['main.js'].replace(
        "import { ready } from './barrel.js';",
        "import * as api from './barrel.js'; globalThis.api = api; const ready = api.ready;",
      ),
    },
  },
  ...['named', 'bare'].map((kind): Scenario => ({
    name: `a removed ${kind} import`,
    modules: {
      'barrel.js': `${sources['barrel.js']}\n${
        kind === 'named' ? "import { flag } from './unused.js';" : "import './unused.js';"
      }`,
      'unused.js': `export const flag = { valid: true };`,
      'product.js': `
        import { flag } from './unused.js';
        ${sources['product.js'].replace('return extra(null);', 'return flag.valid && extra(null);')}
      `,
    },
    unloaded: ['unused.js'],
  })),
];

function staticClosure(chunks: OutputChunk[], entry: OutputChunk): Set<string> {
  const byFile = new Map(chunks.map((chunk) => [chunk.fileName, chunk]));
  const pending = [entry.fileName];
  const files = new Set(pending);
  for (const file of pending) {
    for (const imported of byFile.get(file)!.imports) {
      expect(byFile.has(imported)).toBe(true);
      if (!files.has(imported)) {
        files.add(imported);
        pending.push(imported);
      }
    }
  }
  return files;
}

async function check(
  scenario: Scenario,
  onDemandWrapping: boolean,
  minify = false,
  chunkOptimization = true,
) {
  const modules = Object.fromEntries(
    Object.entries({ ...sources, ...scenario.modules }).map(([id, code]) => [prefix + id, code]),
  );
  const bundle = await rolldown({
    input: { main: prefix + 'main.js' },
    preserveEntrySignatures: false,
    experimental: { onDemandWrapping, chunkOptimization },
    plugins: [
      {
        name: 'loading-modules',
        resolveId(id) {
          if (id.startsWith('./')) return prefix + id.slice(2);
          return Object.hasOwn(modules, id) ? id : null;
        },
        load(id) {
          return modules[id] ?? null;
        },
      },
    ],
  });
  let chunks: OutputChunk[];
  try {
    const { output } = await bundle.generate({
      format: 'es',
      strictExecutionOrder: true,
      minify,
      entryFileNames: '[name].mjs',
      chunkFileNames: '[name]-[hash].mjs',
    });
    chunks = output.filter((item): item is OutputChunk => item.type === 'chunk');
  } finally {
    await bundle.close();
  }
  const main = chunks.find((chunk) => chunk.facadeModuleId === prefix + 'main.js')!;
  expect(main).toBeDefined();
  const initial = staticClosure(chunks, main);
  for (const id of ['product.js', ...(scenario.unloaded ?? [])]) {
    expect(
      chunks.some(
        (chunk) =>
          initial.has(chunk.fileName) && (chunk.modules[prefix + id]?.renderedLength ?? 0) > 0,
      ),
      id,
    ).toBe(false);
  }
  const directory = await mkdtemp(path.join(tmpdir(), 'rolldown-loading-'));
  try {
    await Promise.all(
      chunks.map((chunk) => writeFile(path.join(directory, chunk.fileName), chunk.code)),
    );
    const runtime = JSON.parse(
      execFileSync(
        process.execPath,
        [
          '--input-type=module',
          '-e',
          `
            import { pathToFileURL } from 'node:url';
            globalThis.events = [];
            await import(pathToFileURL(process.argv[1]));
            const before = {
              value: globalThis.startupValue,
              events: [...globalThis.events],
              productCalls: globalThis.productCalls ?? 0,
            };
            const first = await globalThis.loadProduct();
            const second = await globalThis.loadProduct();
            const state = await globalThis.loadLegacy();
            state.DEFAULT = 'changed';
            const third = await globalThis.loadProduct();
            console.log(JSON.stringify({ before, first, second, third,
              events: globalThis.events, productCalls: globalThis.productCalls }));
          `,
          path.join(directory, main.fileName),
        ],
        { encoding: 'utf8' },
      ),
    );
    const events = scenario.events ?? ['used', 'barrel', 'main'];
    expect(runtime).toEqual({
      before: { value: scenario.initialValue ?? 'ready', events, productCalls: 0 },
      first: scenario.productValue ?? 'fallback',
      second: scenario.productValue ?? 'fallback',
      third: 'changed',
      events,
      productCalls: 3,
    });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

for (const onDemandWrapping of [false, true]) {
  const mode = onDemandWrapping ? 'on-demand' : 'wrap-all';
  test.each(scenarios)(`${mode} preserves lazy loading with $name`, (scenario) =>
    check(scenario, onDemandWrapping),
  );
  test.each([false, true])(
    `${mode} preserves lazy loading with minification and chunkOptimization=%s`,
    (chunkOptimization) => check(scenarios[1], onDemandWrapping, true, chunkOptimization),
  );
}
