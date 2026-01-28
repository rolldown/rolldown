import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import {
  viteDynamicImportVarsPlugin,
  viteImportGlobPlugin,
} from 'rolldown/experimental';

export default defineTest({
  skip: true,
  config: {
    plugins: [
      viteDynamicImportVarsPlugin({
        async resolver(id) {
          return id.replace('@', path.resolve(import.meta.dirname, './dir/a'));
        },
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
