const a = require('./a.js');
const b = require('./b.js');

import assert from 'node:assert';

assert.strictEqual(a.before, 'a-before');
assert.strictEqual(a.after, 'a-after');
assert.strictEqual(a.seenB, 'b');

assert.strictEqual(b.b, 'b');
assert.strictEqual(b.seenABefore, 'a-before');

assert.strictEqual(b.seenAObject, a);
