import assert from 'node:assert';

globalThis.__events = [];

const entry = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.__events, ['main']);

assert.strictEqual((await entry.loadA()).value, 'shared');
assert.deepStrictEqual(globalThis.__events, ['main', 'shared-cjs', 'route-a']);

assert.strictEqual((await entry.loadB()).value, 'shared');
assert.deepStrictEqual(globalThis.__events, ['main', 'shared-cjs', 'route-a', 'route-b']);
