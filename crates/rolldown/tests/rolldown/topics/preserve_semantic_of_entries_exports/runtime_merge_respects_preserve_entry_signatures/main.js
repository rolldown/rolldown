// Second entry: reads lib's exports (a cross-chunk static edge to `lib`) and
// independently pulls runtime helpers via interop. `lib` also pulls runtime
// helpers, making it possible for the runtime to merge into `lib` when entry
// signature extension is allowed.
import assert from 'node:assert';
import { foo, bar } from './lib.js';

assert.strictEqual(foo, 'foo_value');
assert.strictEqual(bar, 42);

export { foo, bar };
