import assert from 'node:assert/strict';
const called = new Promise((resolve) => {
  globalThis.thenCalled = resolve;
});
const { load } = await import('./dist/main.js');
assert.equal(await load(), 'assimilated:expected');
await called;
assert.equal(globalThis.thenCalls, 1);
