import assert from 'node:assert';

globalThis.__events = [];

const entry = await import('./dist/main.js');

assert.deepStrictEqual(globalThis.__events, ['cn', 'clone', 'main:cn']);

const route = await entry.loadRoute();

assert.deepStrictEqual(route.default, { value: 1 });
assert.deepStrictEqual(globalThis.__events, ['cn', 'clone', 'main:cn', 'route']);
