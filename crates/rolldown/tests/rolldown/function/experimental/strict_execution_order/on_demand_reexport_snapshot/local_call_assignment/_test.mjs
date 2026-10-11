import assert from 'node:assert/strict';
import { result, read } from './dist/main.js';

assert.equal(result, 'local');
assert.equal(await read(), 'before');
