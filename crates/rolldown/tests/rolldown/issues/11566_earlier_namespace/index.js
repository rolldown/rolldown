import * as early from './first.js';
export { value } from './forward.js';
import assert from 'node:assert/strict';

assert.equal(early.value, false);
