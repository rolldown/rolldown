import assert from 'node:assert';
import * as cjs from './cjs.js';

// A name with a lone surrogate is treated like a computed one, so its property survives.
assert.strictEqual(cjs['\uD800'], 'assigned');
assert.strictEqual(cjs['\uDC00'], 'defined');
assert.strictEqual(cjs.foo, 'foo');
