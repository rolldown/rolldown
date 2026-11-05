import assert from 'node:assert';
import { after, before, x } from './dist/main.js';

assert.deepStrictEqual(before, ['tla start', 'tla end']);
assert.strictEqual(x, 1);
assert.deepStrictEqual(after, ['tla start', 'tla end', 'imports-tla']);
