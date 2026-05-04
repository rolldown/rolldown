import assert from 'node:assert';
import './shim.js';

import.meta.hot.accept('./shim.js', () => {});

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(
    globalThis.__cjs_requires_esm_interop_patch_ran,
    true,
    'shim.js HMR patch should have executed',
  );
});
