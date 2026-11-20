import assert from 'node:assert';

globalThis.order = [];
const m = await import('./dist/main.js');

assert.deepStrictEqual(globalThis.order, [
  'c sees function',
  'a sees function',
  'y',
  'b sees foo',
  'z',
]);
assert.strictEqual(m.access(), 'foo');
