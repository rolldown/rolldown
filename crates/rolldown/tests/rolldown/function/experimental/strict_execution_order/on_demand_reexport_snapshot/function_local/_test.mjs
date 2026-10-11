import assert from 'node:assert/strict';
import { result, read, readExtra } from './dist/main.js';

assert.equal(result, 'local');
assert.deepEqual(await Promise.all([read(), read()]), ['before', 'before']);
assert.equal(await readExtra(), 'extra');
