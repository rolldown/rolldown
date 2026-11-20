import assert from 'node:assert/strict';

export { result as ok } from './middle.js';
import { result } from './middle.js';

assert.equal(result, true);
