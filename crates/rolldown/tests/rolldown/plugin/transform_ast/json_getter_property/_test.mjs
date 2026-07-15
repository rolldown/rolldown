import assert from 'node:assert/strict';
import { data, first, normal, second } from './dist/main.js';

assert.equal(normal, 4);
assert.equal(first, 4);
assert.equal(second, 4);
assert.equal(data.stable, 1);
assert.equal(globalThis.jsonGetterReads, 3);
