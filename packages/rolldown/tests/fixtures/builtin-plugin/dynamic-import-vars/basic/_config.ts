import { isSingleThread } from '@tests/runtime-flavor';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteDynamicImportVarsPlugin, viteImportGlobPlugin } from 'rolldown/experimental';

export default defineTest({
  // KNOWN: single-thread block_on hazard. The JS `resolver` below is awaited
  // under `block_on` in ViteDynamicImportVarsPlugin::transform (lib.rs:122);
  // on the CurrentThread runtime the transform runs on the (blocked) JS
  // thread, so the TSFN continuation can never fire -> genuine deadlock. The
  // runtime now detects this park itself and panics with the typed
  // `BlockOnDeadlock` diagnostic (rolldown_utils::async_runtime -- always-on
  // on threadless wasm, armed via ROLLDOWN_PARK_DEADLINE_MS in the native
  // single-thread lane), so a hang is LOUD instead of freezing vitest -- but
  // detection does not make the plugin work, so this stays skipped.
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
