import assert from 'node:assert';

const { app } = await import('./dist/main.js');
const appNs = await app;

assert.deepEqual(Object.keys(appNs).sort(), ['done', 'fromA', 'fromB', 'own']);
assert.strictEqual(appNs.fromA, 'a');
assert.strictEqual(appNs.fromB, 'b');
assert.strictEqual(appNs.own, 43);
assert.strictEqual(await appNs.done, 84);
