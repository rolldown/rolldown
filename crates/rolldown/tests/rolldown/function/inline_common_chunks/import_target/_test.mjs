import assert from 'node:assert';

// `y` loads `m.js` through `import()` before `x`, which carries its own copy, runs.
globalThis.events = [];
await import('./dist/y.js');
const ns = await globalThis.loadM();
await import('./dist/x.js');

assert.deepStrictEqual(globalThis.events, ['M body', 'Y 1', 'X 2']);
assert.strictEqual(Object.getPrototypeOf(ns), null);
assert.strictEqual(ns[Symbol.toStringTag], 'Module');
assert.strictEqual(ns.count, 2, 'x and y share one instance of m.js');
assert.strictEqual(await globalThis.loadM(), ns);
