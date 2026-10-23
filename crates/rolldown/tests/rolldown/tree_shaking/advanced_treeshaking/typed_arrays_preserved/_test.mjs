import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_element_value_of',
  'LIVE_negative_length',
  'LIVE_large_length',
  'LIVE_bigint_array',
  'LIVE_written_source',
  'LIVE_array_like_length',
  'LIVE_shadowed_constructor',
  'LIVE_typed_value',
]);
