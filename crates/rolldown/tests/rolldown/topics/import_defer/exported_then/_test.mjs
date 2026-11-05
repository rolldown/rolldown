import assert from 'node:assert';
import * as m from './dist/main.js';

assert.strictEqual(m.descriptor, undefined);
assert.strictEqual(m.hasThen, false);
assert.deepStrictEqual(m.before, []);
assert.deepStrictEqual(m.keys, ['foo', 'Symbol(Symbol.toStringTag)']);
assert.deepStrictEqual(m.after, ['x']);
