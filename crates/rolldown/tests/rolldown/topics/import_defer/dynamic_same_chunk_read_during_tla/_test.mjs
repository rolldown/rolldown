import assert from 'node:assert';
import { early, run, seenAfterPause } from './dist/main.js';

assert.deepStrictEqual(early, [
  ['d', ['a']],
  ['data', 1],
]);
assert.strictEqual(seenAfterPause, 2);
assert.deepStrictEqual(await run(), {
  same: true,
  tag: '[object Deferred Module]',
  jsonSame: true,
  jsonTag: '[object Deferred Module]',
});
