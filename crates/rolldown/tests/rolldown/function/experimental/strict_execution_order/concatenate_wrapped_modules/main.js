import assert from 'node:assert'
import './setup.js';
import './foo.js';
import v from './foo_inner.js'
assert.equal(v.foo, 'foo');

