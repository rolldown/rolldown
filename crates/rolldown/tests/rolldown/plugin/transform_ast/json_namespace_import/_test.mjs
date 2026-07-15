import assert from 'node:assert/strict';
import { after, before, data } from './dist/main.js';

assert.equal(globalThis.jsonNamespaceSideEffectRan, true);
assert.equal(before, 4);
assert.equal(after, 4);
assert.equal(data.normal, 4);
assert.equal(data['property-name'], 2);
assert.deepEqual(data.default, { normal: 9, 'property-name': 2 });
