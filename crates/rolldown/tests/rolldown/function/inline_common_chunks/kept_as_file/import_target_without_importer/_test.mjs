import assert from 'node:assert';

// No file imports `m.js` after tree shaking, so `import()` loads it as an ordinary file.
globalThis.events = [];
await import('./dist/x.js');
await import('./dist/y.js');
const ns = await globalThis.loadM();

assert.deepStrictEqual(globalThis.events, ['X']);
assert.strictEqual(ns.inc(), 1);
assert.strictEqual(ns.count, 1);
