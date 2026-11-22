import assert from 'node:assert';

globalThis.log = [];
const { p, readD, readR } = await import('./dist/main.js');
// `b.js` imports `l.js`, then `w.js`. The deferred imports evaluate nothing.
assert.deepStrictEqual(globalThis.log, ['l', 'w', 'b']);
assert.strictEqual(readR(), 'rm');
assert.deepStrictEqual(globalThis.log, ['l', 'w', 'b', 'm', 'r']);
const ns = await p;
assert.strictEqual(ns.d, 'dl');
assert.strictEqual(readD(), 'dl');
assert.deepStrictEqual(globalThis.log, ['l', 'w', 'b', 'm', 'r', 'd']);
