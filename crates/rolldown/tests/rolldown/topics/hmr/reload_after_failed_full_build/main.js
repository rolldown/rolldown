import assert from 'node:assert';
import './hmr.js';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  // Only a reload after the full build of step 1 runs `v3`: no patch carries it.
  assert.strictEqual(globalThis.__reload_after_failed_full_build_value, 'v3');
});
