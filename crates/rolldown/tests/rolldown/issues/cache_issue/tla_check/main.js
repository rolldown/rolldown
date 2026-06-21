import { libValue } from './lib.js';
import assert from 'node:assert';

assert.strictEqual(libValue, 'dep+lib');
