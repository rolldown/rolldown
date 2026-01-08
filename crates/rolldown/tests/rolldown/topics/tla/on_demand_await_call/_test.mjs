import assert from 'node:assert';
import { normalLibValue, tlaLibValue } from './dist/main.js';

assert.strictEqual(normalLibValue, 'normal-dep+normal-lib');
assert.strictEqual(tlaLibValue, 'tla-dep+tla-lib');
