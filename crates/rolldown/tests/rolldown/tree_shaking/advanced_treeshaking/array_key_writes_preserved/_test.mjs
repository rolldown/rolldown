import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_same_index',
  'LIVE_string_key',
  'LIVE_unknown_key',
  'LIVE_truncate',
  'LIVE_extend',
  'LIVE_past_end_length',
  'LIVE_delete_element',
  'LIVE_delete_length',
  'LIVE_custom_key',
  'LIVE_negative_key',
  'LIVE_non_index_key',
  'LIVE_alias_index',
]);
