import assert from 'node:assert';

globalThis.log = [];
const { read } = await import('./dist/main.js');
// `b.js` imports `a1.js`, `w1.js`, `a2.js`, then `w2.js`.
assert.deepStrictEqual(globalThis.log, ['a1', 'w1', 'a2', 'w2', 'b']);
assert.strictEqual(read(), 'a1a2');
