import assert from 'node:assert/strict';

// `a.js` imports the entry `b.js`, and shares `s.js` with `d.js`, which `b.js` loads with
// `import()`. Loading `b.js` must not run `a.js`.
globalThis.log = [];
const b = await import('./dist/b.js');
assert.strictEqual(await b.loaded, 'd:s');
assert.deepStrictEqual(globalThis.log, []);
