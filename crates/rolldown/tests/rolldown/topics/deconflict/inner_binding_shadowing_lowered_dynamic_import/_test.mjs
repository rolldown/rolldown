import { strict as assert } from 'node:assert';
import { lib, nested } from './dist/main.js';
assert.equal(lib.local, 'local');
assert.equal(lib.object, 'object');
assert.equal((await lib.load()).value, 'dep');
assert.equal((await nested(1)).value, 'dep');
