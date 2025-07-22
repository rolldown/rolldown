globalThis.globalVar = true;
const foo = require('./foo')
import assert from 'node:assert'


assert.strictEqual(foo.a1, 1000);
assert.strictEqual(foo.a2, 'baz');
assert.strictEqual(foo.a3, 1000);
assert.strictEqual(foo.destructuring, 'destructuring');
assert.strictEqual(foo.index, 10);
