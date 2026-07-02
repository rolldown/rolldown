import { isSingleThread } from '@tests/runtime-flavor';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteDynamicImportVarsPlugin, viteImportGlobPlugin } from 'rolldown/experimental';

export default defineTest({
  // KNOWN: single-thread block_on hazard. The JS `resolver` below is awaited
  // under `block_on` in ViteDynamicImportVarsPlugin::transform (lib.rs:122);
  // on the CurrentThread runtime the transform runs on the (blocked) JS
  // thread, so the TSFN continuation can never fire -> hard deadlock that not
  // even the vitest timeout can interrupt.
  skip: isSingleThread,
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
