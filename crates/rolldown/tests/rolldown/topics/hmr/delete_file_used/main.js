import assert from 'node:assert';
import './parent.js';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(
    globalThis.__delete_file_used_parent_reran,
    true,
    'step 1 (recreate child.js with changed content) should have re-run parent.js',
  );
});
