import { bar as b1, value1 } from './lib1.js';
import { bar as b2, value2 } from './lib2.js';

import assert from 'node:assert';

assert.strictEqual(value1, 'lib1-value');
assert.strictEqual(b1, 'vendor-bar');
assert.strictEqual(value2, 'lib2-value');
assert.strictEqual(b2, 'vendor-bar');
