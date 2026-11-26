import assert from 'node:assert/strict';
import { value, change } from './dist/entry.js';

change();
assert.equal(value, 2);
