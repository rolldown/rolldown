import path from 'node:path';
import type { OutputAsset, OutputBundle, OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Rollup lets a plugin keep the bundle past `generateBundle` / `writeBundle`.
// On threadless WASI the hook's native boxes are dropped when it settles, so
// every lazy field must already be cached by then. Nothing below reads a lazy
// field during the hooks; `afterTest` reads them all late.
const kept: { hook: string; bundle: OutputBundle }[] = [];
let deletedAsset: OutputAsset | undefined;

const entry = path.join(__dirname, 'main.js');
const dep = path.join(__dirname, 'dep.js');

function expectReadable(hook: string, bundle: OutputBundle): void {
  const main = bundle['main.js'] as OutputChunk;
  expect(main.type, hook).toBe('chunk');
  expect(main.code, hook).toContain('kept-output-bundle');
  expect(main.map?.mappings, hook).toEqual(expect.any(String));
  expect(main.moduleIds, hook).toContain(entry);
  expect(Object.keys(main.modules), hook).toContain(dep);
  expect(main.imports, hook).toEqual([]);
  expect(main.dynamicImports, hook).toEqual(expect.any(Array));
  const asset = bundle['kept.txt'] as OutputAsset;
  expect(asset.source, hook).toBe('kept asset');
}

export default defineTest({
  // Module-level state shared between the hooks and `afterTest`.
  sequential: true,
  config: {
    input: entry,
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'keep-output-bundle',
        buildStart() {
          this.emitFile({ type: 'asset', fileName: 'kept.txt', source: 'kept asset' });
          this.emitFile({ type: 'asset', fileName: 'deleted.txt', source: 'deleted asset' });
        },
        generateBundle(_options, bundle) {
          kept.push({ hook: 'generateBundle', bundle });
          // A plugin may keep an item it deletes from the bundle.
          deletedAsset = bundle['deleted.txt'] as OutputAsset;
          delete bundle['deleted.txt'];
        },
        writeBundle(_options, bundle) {
          kept.push({ hook: 'writeBundle', bundle });
        },
      },
    ],
  },
  beforeTest() {
    kept.length = 0;
    deletedAsset = undefined;
  },
  afterTest() {
    expect(kept.map(({ hook }) => hook)).toStrictEqual(['generateBundle', 'writeBundle']);
    for (const { hook, bundle } of kept) {
      expectReadable(hook, bundle);
    }
    expect(deletedAsset?.source).toBe('deleted asset');
    // A late write has no effect on the output, but must not throw either.
    for (const { bundle } of kept) {
      (bundle['main.js'] as OutputChunk).code += '';
      (bundle['kept.txt'] as OutputAsset).source = 'late write';
    }
  },
});
