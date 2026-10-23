import assert from 'node:assert';

import { done, lastIndex } from './dist/main.js';
assert.strictEqual(lastIndex(), 1);
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_test_last_index',
  'LIVE_exec_last_index',
  'LIVE_holder_last_index',
  'LIVE_alias_last_index',
  'LIVE_own_exec',
  'LIVE_last_index_value_of',
  'LIVE_test_argument',
  'LIVE_escaped_last_index',
]);
