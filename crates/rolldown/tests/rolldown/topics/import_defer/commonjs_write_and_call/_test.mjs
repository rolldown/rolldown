import assert from 'node:assert';
import { afterDelete, afterSet, calls, deleteThrows, objValue, setThrows } from './dist/main.js';

assert.strictEqual(setThrows, true);
assert.deepStrictEqual(afterSet, []);
assert.strictEqual(deleteThrows, true);
assert.deepStrictEqual(afterDelete, ['t']);
assert.strictEqual(objValue, 2);
assert.deepStrictEqual(calls, [true, true, true, true, true]);
