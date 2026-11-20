import assert from 'node:assert';
import * as m from './dist/main.js';

assert.strictEqual(m.setError, 'TypeError');
assert.deepStrictEqual(m.afterSet, []);
assert.strictEqual(m.updateError, 'TypeError');
assert.deepStrictEqual(m.afterUpdate, ['x']);
assert.strictEqual(m.deleteError, 'TypeError');
assert.strictEqual(m.value, 1);
