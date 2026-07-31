import assert from 'node:assert';

const { app } = await import('./dist/main.js');
const appNs = await app;

assert.strictEqual(typeof appNs.sep, 'string');
assert.strictEqual(appNs.own, 43);
assert.strictEqual(await appNs.done, 84);
