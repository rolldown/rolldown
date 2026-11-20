import assert from 'node:assert';
import { log, result } from './dist/main.js';

assert.strictEqual(result, 'a failed');
assert.deepStrictEqual(log, ['a', 'import(a) rejects', 'import.defer resolves']);
