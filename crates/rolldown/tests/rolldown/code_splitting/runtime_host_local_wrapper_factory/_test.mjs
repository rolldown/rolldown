import assert from 'node:assert/strict';

globalThis.calls = 0;
await assert.rejects(import('./dist/main.js'), { message: 'entry failed' });
const [first, second] = await globalThis.results;
assert.deepEqual(first, { value: 42 });
assert.equal(first, second);
assert.equal(globalThis.calls, 1);
