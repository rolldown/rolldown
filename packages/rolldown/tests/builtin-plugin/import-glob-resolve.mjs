import assert from 'node:assert/strict';
import path from 'node:path';
import { rolldown } from 'rolldown';
import { viteImportGlobPlugin } from 'rolldown/experimental';

const root = process.argv[2];
const bundle = await rolldown({
  input: path.join(root, 'main.js'),
  plugins: [
    {
      // A JS `resolveId` hook that calls `this.resolve()`, like SvelteKit's guard plugin.
      name: 'resolve-again',
      resolveId: {
        order: 'pre',
        async handler(id, importer, options) {
          if (importer) await this.resolve(id, importer, options);
        },
      },
    },
    {
      name: 'alias',
      resolveId(id) {
        if (id.startsWith('@data/')) return path.join(root, 'data', id.slice('@data/'.length));
      },
    },
    viteImportGlobPlugin({ root }),
  ],
});
const { output } = await bundle.generate();
assert.match(output[0].code, /globbed-data/);
