import assert from 'node:assert/strict';
import { local, load } from './dist/main.js';

assert.equal(local, 'local');
assert.deepEqual(await load(), [0, 'pure']);
