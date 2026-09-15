import assert from 'node:assert';

await import('./dist/main.js');
await new Promise((resolve) => setImmediate(resolve));

assert.strictEqual(globalThis.sideEffect.touched, true);
