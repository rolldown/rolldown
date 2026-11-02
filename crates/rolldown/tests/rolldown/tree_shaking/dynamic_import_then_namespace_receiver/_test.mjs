import assert from 'node:assert/strict';
const called = new Promise((resolve) => {
  globalThis.thenCalled = resolve;
});
const { load, loadInFunction } = await import('./dist/main.js');
assert.equal(await load(), 'assimilated:expected');
assert.equal(await loadInFunction(), 'loaded');
assert.equal(globalThis.inFunctionThenCalls, 1);
await called;
assert.equal(globalThis.keptThenCalls, 1);
assert.equal(globalThis.droppedThenCalls, undefined);
assert.equal(globalThis.droppedCjsThenCalls, undefined);
