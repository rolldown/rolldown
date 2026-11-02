import assert from 'node:assert';
import './dist/main.js';

const e = await import('./dist/e.js');
assert.strictEqual(e.value, 123);
assert.strictEqual(e.renamed, 123);
assert.strictEqual(e.default.value, 123);
assert.strictEqual(e.default.getA().value, 123);
