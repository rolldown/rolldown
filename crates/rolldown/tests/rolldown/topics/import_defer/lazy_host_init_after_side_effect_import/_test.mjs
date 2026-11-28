import assert from 'node:assert';

globalThis.log = [];
const { read } = await import('./dist/main.js');
// `b.js` imports `a1.js`, `w.js`, then `a2.js`.
assert.deepStrictEqual(globalThis.log, ['a1', 'w', 'a2', 'b']);
assert.strictEqual(read(), 'a1a2');
