import assert from 'node:assert/strict';

const originalNow = Date.now;
let tick = 0;
Date.now = () => ++tick;

try {
  const { loadA, loadB } = await import('./dist/main.js');
  const first = await loadA();
  assert.ok(first.AValue < first.BValue);
  assert.equal(first.LOCAL_A(), 'a');
  const second = await loadB();
  assert.equal(second.AValue, first.AValue);
  assert.equal(second.BValue, first.BValue);
  assert.equal(second.LOCAL_B(), 'b');
} finally {
  Date.now = originalNow;
}
