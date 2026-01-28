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
        async resolver(id) {
          return id.replace('@', path.resolve(import.meta.dirname, './dir/a'));
        },
      }),
      viteImportGlobPlugin(),
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
