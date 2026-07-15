import assert from 'node:assert/strict';
import { after, before, data } from './dist/main.js';

assert.equal(globalThis.jsonMutationSideEffectRan, true);
assert.equal(before, 4);
assert.equal(after, 4);
assert.equal(data.normal, 9);
