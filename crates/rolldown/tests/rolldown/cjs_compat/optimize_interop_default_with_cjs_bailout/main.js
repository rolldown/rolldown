import mod from './commonjs.js';
import assert from 'node:assert/strict';

assert.deepEqual(mod.slice(1), [2, 3], 'should import JSON file as expected');

assert.equal(mod.foo, 1);
