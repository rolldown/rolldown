import assert from 'node:assert';

const { ns, loadLazy } = await import('./dist/other.js');
assert.strictEqual(ns.a, 1);
assert.strictEqual((await loadLazy()).ns2.b, 2);
const { hello } = await import('./dist/main.js');
assert.strictEqual(hello, 'hi');
