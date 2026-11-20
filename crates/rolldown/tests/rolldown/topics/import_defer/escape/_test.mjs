import assert from 'node:assert';
import * as m from './dist/main.js';

assert.strictEqual(m.same, true);
assert.strictEqual(m.tag, '[object Deferred Module]');
assert.deepStrictEqual(m.before, []);
assert.deepStrictEqual(m.keys, ['count', 'foo', 'inc']);
assert.deepStrictEqual(m.after, ['x']);
assert.strictEqual(m.count, 1);
