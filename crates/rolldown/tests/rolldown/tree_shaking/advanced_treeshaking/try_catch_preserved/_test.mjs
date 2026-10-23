import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_map_entry',
  'LIVE_weak_set',
  'LIVE_keys_null',
  'LIVE_typed_array_length',
  'LIVE_callback',
  'LIVE_block',
  'LIVE_argument',
  'LIVE_property',
  'LIVE_conditional',
  'LIVE_logical',
  'LIVE_function',
  'LIVE_after_folded_branch',
]);
