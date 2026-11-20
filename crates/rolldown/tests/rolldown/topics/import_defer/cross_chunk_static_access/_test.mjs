import assert from 'node:assert';
import { log, value } from './dist/main.js';

assert.strictEqual(value, 'd:2');
assert.deepStrictEqual(log, ['main', 'd']);
