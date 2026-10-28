import assert from 'node:assert';
import a from './a.cjs';

assert.strictEqual(a.name, 'a');
assert.strictEqual(a.other(), 'b');
assert.strictEqual(a.partial, 'object');
