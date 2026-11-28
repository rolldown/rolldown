import assert from 'node:assert';
import './a.js';
import './b.js';

// Step 0 edits `a.js`, and the client does not get that push. Step 1 edits `b.js`. Its push
// skips a generation, so the client pulls with the old generation and gets a catch-up that
// also names `a.js`. Both edits must run.
process.on('beforeExit', (code) => {
  if (code !== 0) return;
  const runs = globalThis.__catch_up_after_lost_push;
  assert.deepStrictEqual(runs.slice(0, 2), ['a v1', 'b v1']);
  assert.deepStrictEqual(runs.slice(2).sort(), ['a v2', 'b v2']);
});
