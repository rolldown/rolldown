import assert from 'node:assert';

globalThis.__events = [];
const main = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.__events, []);
assert.strictEqual(main.run(), 1);
const other = await import('./dist/other.js');
assert.deepStrictEqual(globalThis.__events, ['other']);
// Native V8 gives 3: `b.js` initializes at the first deferred access, after the top level of
// `main.js` set `state.value = 3`, and before `run()` sets 2.
assert.strictEqual(other.other(), 3);
