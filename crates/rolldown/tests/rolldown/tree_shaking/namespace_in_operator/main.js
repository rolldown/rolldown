import assert from 'node:assert';
import * as foo from './foo.js';

function hasKey(foo) {
  return 'a' in foo;
}

if ('d' in foo) foo.d();
assert.strictEqual('c' in foo ? foo.c() : 'missing', 'c');
assert.strictEqual('a' in foo, true);
assert.strictEqual('e' in foo, true);
assert.strictEqual('default' in foo, false);
assert.strictEqual(hasKey({}), false);
