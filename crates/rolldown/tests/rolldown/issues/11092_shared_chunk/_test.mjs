import assert from 'node:assert';

await import('./dist/b.js');
assert.ok(globalThis.extEvaluated, '`ext` must be evaluated');

const a = await import('./dist/a.js');
assert.strictEqual(a.value, 'ext');
