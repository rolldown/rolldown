import assert from 'node:assert';
import b from './bridge.js';

// Initial bundle: the regular finalizer's `__toCommonJS` rewrite stamps
// `__esModule: true` on the bridge's exports, so `import b` reads the ESM
// default cleanly via `__toESM`.
assert.strictEqual(b, 'esm-default');

import.meta.hot.accept('./bridge.js', (mod) => {
  // The HMR finalizer's `try_rewrite_require` must also wrap with
  // `__toCommonJS`, otherwise the bridge's exports would be the raw ESM
  // namespace post-patch and a downstream `import x from './bridge.js'`
  // would have its `__toESM` wrap wrongly to the whole namespace instead of the
  // default value.
  assert.strictEqual(mod.__esModule, true);
  assert.strictEqual(mod.default, 'esm-default');
  globalThis.__cjs_bridge_esm_default_patch_verified = true;
});

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(
    globalThis.__cjs_bridge_esm_default_patch_verified,
    true,
    'bridge.js HMR patch accept handler should have run and verified',
  );
});
