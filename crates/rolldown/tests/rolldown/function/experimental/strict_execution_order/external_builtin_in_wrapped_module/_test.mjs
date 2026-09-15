import assert from 'node:assert';
import { out } from './dist/main.js';

assert.strictEqual(out, 'dep:true');
assert.strictEqual(globalThis.__depRan, true);
