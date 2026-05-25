import { createRequire } from 'node:module';
import assert from 'node:assert';

const require = createRequire(import.meta.url);
const { v4 } = require('./dist/index.js');

assert.strictEqual(typeof v4, 'function', 'v4 should be a function, not the namespace wrapper');
assert.strictEqual(v4(), 'uuid-here');
