import assert from 'node:assert/strict';
import { local, load } from './dist/main.js';

assert.equal(local, 'local');
assert.deepEqual(await Promise.all([load(), load()]), ['lazy', 'lazy']);
