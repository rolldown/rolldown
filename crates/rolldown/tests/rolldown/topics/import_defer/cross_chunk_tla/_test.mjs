import assert from 'node:assert';
import { log, value } from './dist/main.js';

assert.strictEqual(value, 'd:3');
assert.deepStrictEqual(log, ['t', 'main', 'd']);
