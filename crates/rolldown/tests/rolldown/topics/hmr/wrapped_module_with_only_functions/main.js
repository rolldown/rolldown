import assert from 'node:assert';
import { triple } from './imported.js';

const { double } = require('./required.js');

assert.strictEqual(double(2), 4);
assert.strictEqual(triple(2), 6);
