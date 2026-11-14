import assert from 'node:assert';
import './hub.js';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(globalThis.__mixed_runs_a, 2, 'a.js must re-run only at step 2 (real change)');
  assert.strictEqual(globalThis.__mixed_runs_b, 2, 'b.js must re-run only at step 0 (real change)');
  assert.strictEqual(
    globalThis.__mixed_runs_hub,
    3,
    'hub.js must re-run at steps 0 and 2, not at step 1',
  );
});
