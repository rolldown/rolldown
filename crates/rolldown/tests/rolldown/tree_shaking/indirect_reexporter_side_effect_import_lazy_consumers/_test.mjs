import assert from 'node:assert/strict';

globalThis.configureCount = 0;
await import('./dist/main.js');

assert.equal(globalThis.configureCount, 0);
const { a } = await globalThis.loadA();
assert.equal(a(), 'a:1.0.0');
const { b } = await globalThis.loadB();
assert.equal(b(), 'b:1.0.0:other');
assert.equal(globalThis.configureCount, 1);
