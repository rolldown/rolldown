import assert from 'node:assert';

// The file `y` loads for `m.js` registers `m.js` and `dep.js`, which `m.js` reads.
globalThis.events = [];
await import('./dist/y.js');
const ns = await globalThis.loadM();
ns.inc();
await import('./dist/z.js');
await import('./dist/x.js');

assert.deepStrictEqual(globalThis.events, ['dep body', 'M body dep', 'Z dep', 'X 2']);
assert.strictEqual(ns.count, 2);
