import assert from 'node:assert';
import { after, before, getB, loadOther, results } from './dist/main.js';

// Both interop views stay distinct, including when execution-order wrappers separate importers.
assert.deepStrictEqual(before, []);
assert.deepStrictEqual(results, {
  bIsC1: true,
  c1IsC2: true,
  sameReceiver: true,
  foo: 'foo',
  defaultTypes: ['string', 'object'],
  wrappedFoo: 'foo',
});
assert.deepStrictEqual(after, ['x']);
const other = await loadOther();
// Another chunk imports two of the shared objects, and the first object of the group (`b.js`)
// stays in its chunk. The `cjs` variant reads each object through its export name.
assert.strictEqual(other.getOne(), getB());
assert.strictEqual(other.getTwo(), other.getOne());
