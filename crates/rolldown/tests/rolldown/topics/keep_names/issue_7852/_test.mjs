import fn from './dist/main.js';
import assert from 'node:assert';

assert.strictEqual(fn.name, 'default');
assert.strictEqual(fn('32px'), 2);
