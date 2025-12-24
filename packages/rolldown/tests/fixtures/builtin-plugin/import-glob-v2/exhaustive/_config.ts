import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';

export default defineTest({
  config: {
    plugins: [
      viteImportGlobPlugin({
        root: path.resolve(import.meta.dirname),
        isV2: { sourcemap: false },
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
