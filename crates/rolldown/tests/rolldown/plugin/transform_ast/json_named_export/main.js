import assert from 'node:assert/strict';
import value, { injected, old } from './data.json';

assert.equal(old, 1);
assert.deepEqual(value, { old: 1, injected: 1 });
assert.equal(injected, 2);
