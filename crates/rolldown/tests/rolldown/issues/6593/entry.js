import assert from 'node:assert'
const mod = await import('./lib.js');

assert.strictEqual(mod.default().name, 'plugin');
