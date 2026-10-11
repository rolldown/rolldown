import assert from 'node:assert';
import { read } from './dist/main.js';

assert.equal(await read(), 'before');
assert.equal(await read(), 'before');
