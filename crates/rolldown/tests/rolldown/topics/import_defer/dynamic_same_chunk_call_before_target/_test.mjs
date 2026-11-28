import assert from 'node:assert';
import { log, read, run } from './dist/main.js';

assert.deepStrictEqual(await run(), { same: true, tag: '[object Deferred Module]' });
assert.deepStrictEqual(log, []);
assert.strictEqual(read(), 1);
assert.deepStrictEqual(log, ['d']);
