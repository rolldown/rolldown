import { first } from './lib1.js';
import { second } from './lib2.js';

import assert from 'node:assert';

assert.strictEqual(typeof first, 'function');
assert.strictEqual(typeof second, 'function');
