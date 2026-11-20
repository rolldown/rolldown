import assert from 'node:assert';

globalThis.log = [];
const { read } = await import('./dist/main.js');
// `other.js` is also an entry. The chunk of `main.js` loads its code, but does not evaluate it.
assert.deepStrictEqual(globalThis.log, ['main']);
assert.strictEqual(read(), 'other');
assert.deepStrictEqual(globalThis.log, ['main', 'other']);
const other = await import('./dist/other.js');
assert.strictEqual(other.value, 'other');
assert.deepStrictEqual(globalThis.log, ['main', 'other']);
