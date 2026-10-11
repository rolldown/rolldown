import assert from 'node:assert/strict';

const originalNow = Date.now;
let tick = 0;
Date.now = () => ++tick;

try {
  const { loadA, loadB } = await import('./dist/main.js');
  const first = await loadB();
  assert.ok(first.BValue < first.AValue);
  const second = await loadA();
  assert.equal(second.AValue, first.AValue);
  assert.equal(second.BValue, first.BValue);
} finally {
  Date.now = originalNow;
}
