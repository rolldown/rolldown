import assert from 'node:assert'
import inner from './foo_inner.js';

assert.strictEqual(inner.foo, 'foo');
