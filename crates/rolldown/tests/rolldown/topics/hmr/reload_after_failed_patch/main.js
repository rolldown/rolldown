import assert from 'node:assert';
import './hmr.js';
import './other.js';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(globalThis.__reload_after_failed_patch_static, 'static');
  assert.strictEqual(globalThis.__reload_after_failed_patch_dynamic, 'dynamic');
});
