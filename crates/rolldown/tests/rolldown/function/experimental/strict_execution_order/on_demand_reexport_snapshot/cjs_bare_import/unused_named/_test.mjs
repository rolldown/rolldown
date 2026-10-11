import assert from 'node:assert/strict';
import { read } from './dist/main.js';

assert.equal(await read(), 'before');
assert.equal(await read(), 'before');
