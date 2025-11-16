import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import {
  importGlobPlugin,
  viteDynamicImportVarsPlugin,
} from 'rolldown/experimental';

export default defineTest({
  config: {
    plugins: [
      viteDynamicImportVarsPlugin({
        async resolver(id) {
          return id.replace('@', path.resolve(import.meta.dirname, './dir/a'));
        },
      }),
      importGlobPlugin({
        root: path.resolve(import.meta.dirname),
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
