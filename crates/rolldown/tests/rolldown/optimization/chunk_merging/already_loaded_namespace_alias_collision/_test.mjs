import assert from 'node:assert';

const { app } = await import('./dist/main.js');
const appNs = await app;

assert.deepEqual(Object.keys(appNs).sort(), ['app_exports', 'done']);
assert.strictEqual(appNs.app_exports, 43);
assert.strictEqual(await appNs.done, 84);
