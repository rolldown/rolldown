import assert from 'node:assert';

globalThis.log = [];
const { read } = await import('./dist/main.js');
// `b.js` imports `a.js`, then `w.js`.
assert.deepStrictEqual(globalThis.log, ['a', 'w', 'b']);
assert.strictEqual(read(), 'a');
