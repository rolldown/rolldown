import assert from 'node:assert';
import { after, before, hasBar, midValue, same, thenValue, value } from './dist/main.js';

assert.strictEqual(thenValue, undefined);
assert.deepStrictEqual(before, []);
assert.strictEqual(value, 'foo');
assert.strictEqual(hasBar, true);
assert.strictEqual(same, true);
assert.deepStrictEqual(after, ['cjs']);
assert.strictEqual(midValue, 'bar');
