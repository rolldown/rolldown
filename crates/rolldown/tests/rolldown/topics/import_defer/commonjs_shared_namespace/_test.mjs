import assert from 'node:assert';
import { after, before, getB, one, results, two } from './dist/main.js';

// The unwrapped importers in one chunk share one deferred namespace object of `x.cjs` for each
// interop mode, and one namespace for the static accesses. With `strictExecutionOrder`, the
// importers have wrappers, and each record keeps its own objects.
const shared = globalThis.__configName !== 'strict';
assert.deepStrictEqual(before, []);
assert.deepStrictEqual(results, {
  bIsC1: shared,
  c1IsC2: shared,
  sameReceiver: shared,
  foo: 'foo',
  defaultTypes: ['string', 'object'],
  wrappedFoo: 'foo',
});
assert.deepStrictEqual(after, ['x']);
// The entry exports two of the shared objects. The first object of the group (`b.js`) is the
// object of both exports. The `cjs` variant reads each object through its export name.
assert.strictEqual(one === getB(), shared);
assert.strictEqual(two === one, shared);
