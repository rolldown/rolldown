import assert from 'node:assert';
import './parent.js';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  // The parent is a boundary for both deps: each dep re-runs, the parent does not.
  assert.strictEqual(globalThis.aRuns, 2);
  assert.strictEqual(globalThis.bRuns, 2);
  assert.strictEqual(globalThis.parentRuns, 1);
  assert.deepStrictEqual(globalThis.saw, [1, 1]);
});
