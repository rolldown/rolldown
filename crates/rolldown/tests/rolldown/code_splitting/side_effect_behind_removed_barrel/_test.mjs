import assert from 'node:assert';

await import('./dist/entry1.js');
assert.strictEqual(globalThis.effectRuns, 1);
