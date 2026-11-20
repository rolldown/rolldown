import assert from 'node:assert';
import { before, log, same, tag, x } from './dist/main.js';

assert.strictEqual(same, true);
assert.strictEqual(tag, '[object Deferred Module]');
assert.deepStrictEqual(before, []);
assert.strictEqual(x, 1);
assert.deepStrictEqual(log, ['d']);
