import { isSingleThread } from '@tests/runtime-flavor';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteDynamicImportVarsPlugin, viteImportGlobPlugin } from 'rolldown/experimental';

export default defineTest({
  // The public factory rejects JS resolvers on CurrentThread. Keep the skipped
  // fixture from constructing the unsupported plugin during config loading.
  skip: isSingleThread,
  config: {
    plugins: isSingleThread
      ? []
      : [
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
