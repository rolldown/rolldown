import assert from 'node:assert/strict';
import value, { old } from './data.json';

assert.equal(globalThis.jsonStaticImportRan, true);
assert.equal(old, 1);
assert.deepEqual(value, { old: 1 });
