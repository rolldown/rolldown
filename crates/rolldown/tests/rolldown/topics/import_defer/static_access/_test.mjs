import assert from 'node:assert';
import * as m from './dist/main.js';

assert.deepStrictEqual(m.before, []);
assert.strictEqual(m.thenValue, undefined);
assert.strictEqual(m.thenIn, false);
assert.deepStrictEqual(m.afterThen, []);
assert.deepStrictEqual(m.afterRead, ['x']);
assert.strictEqual(m.fooIn, true);
assert.strictEqual(m.value, 3);
assert.strictEqual(m.plainThen, undefined);
assert.deepStrictEqual(m.afterPlainThen, ['x']);
assert.strictEqual(m.inner, 'inner');
assert.deepStrictEqual(m.afterInner, ['x', 'y']);
