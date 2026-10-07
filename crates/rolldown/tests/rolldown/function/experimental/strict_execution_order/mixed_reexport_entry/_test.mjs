import assert from 'node:assert/strict';
import { LOCAL, VALUE, load } from './dist/main.js';

assert.equal(LOCAL, 'local');
assert.deepEqual(VALUE, { value: 'entry' });
const [value, again] = await Promise.all([load(), load()]);
assert.deepEqual(value, { value: 'dynamic' });
assert.equal(value, again);
