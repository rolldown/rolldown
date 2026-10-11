import assert from 'node:assert';

await import('./dist/main.js');
assert.ok(globalThis.extEvaluated, '`ext` must be evaluated');
