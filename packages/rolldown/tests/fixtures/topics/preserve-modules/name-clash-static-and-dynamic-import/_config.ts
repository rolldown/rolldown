import path from 'node:path';
import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// `item.js?static` and `item.js?dynamic` both get the name `item`. Chunks get their names in the
// default order: user-defined entries first, then the other chunks in execution order. So
// `item.js?static`, which `static.js` imports, gets `item.mjs`, and `item.js?dynamic`, which the
// dynamically imported `dynamic.js` imports, gets `item2.mjs`. Rollup gives the same file names.
export default defineTest({
  sequential: true,
  config: {
    output: {
      dir: 'dist',
      preserveModules: true,
      entryFileNames: '[name].mjs',
    },
    plugins: [
      {
        name: 'virtual-item-query',
        resolveId(id, importer) {
          if (importer && id.startsWith('./item.js?')) {
            return path.join(path.dirname(importer), 'item.js') + id.slice('./item.js'.length);
          }
        },
        load(id) {
          const query = /item\.js\?(static|dynamic)$/.exec(id)?.[1];
          if (query) {
            return `(globalThis.nameClashStaticAndDynamicImport ??= []).push('${query}');\nexport default '${query}';`;
          }
        },
      },
    ],
  },
  afterTest: async (output) => {
    const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
    expect(
      Object.fromEntries(
        chunks.map((chunk) => [
          path.basename(chunk.facadeModuleId ?? ''),
          [chunk.fileName, chunk.imports],
        ]),
      ),
    ).toStrictEqual({
      'main.js': ['main.mjs', ['static.mjs']],
      'static.js': ['static.mjs', ['item.mjs']],
      'dynamic.js': ['dynamic.mjs', ['item2.mjs']],
      'item.js?static': ['item.mjs', []],
      'item.js?dynamic': ['item2.mjs', []],
    });
    await import('./_test.mjs');
  },
});
