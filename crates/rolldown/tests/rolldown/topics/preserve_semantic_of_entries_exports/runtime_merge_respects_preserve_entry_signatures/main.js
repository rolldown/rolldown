// Second entry: reads lib's exports (a cross-chunk static edge to `lib`) and
// independently pulls runtime helpers via interop, making `lib` the consumer-set
// dominator the runtime is a candidate to merge into.
import assert from 'node:assert';
import { foo, bar } from './lib.js';

assert.strictEqual(foo, 'foo_value');
assert.strictEqual(bar, 42);

export { foo, bar };
