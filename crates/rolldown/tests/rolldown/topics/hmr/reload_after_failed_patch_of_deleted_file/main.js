import assert from 'node:assert';
import './parent.js';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(globalThis.__reload_after_failed_patch_of_deleted_file_step, 2);
});
