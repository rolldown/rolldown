import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import {
  viteDynamicImportVarsPlugin,
  viteImportGlobPlugin,
} from 'rolldown/experimental';

export default defineTest({
  config: {
    plugins: [
      viteDynamicImportVarsPlugin({
        exclude: [/main\.js$/],
        isV2: { sourcemap: false },
      }),
      viteImportGlobPlugin({
        root: path.resolve(import.meta.dirname),
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
