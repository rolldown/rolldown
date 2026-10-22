import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_index_write',
  'LIVE_new_index_write',
  'LIVE_push',
  'LIVE_pop',
  'LIVE_unshift',
  'LIVE_shift',
  'LIVE_reverse',
  'LIVE_length_write',
  'LIVE_length_read',
  'LIVE_alias_write',
  'LIVE_callback_write',
  'LIVE_spread_write',
  'LIVE_for_of_write',
  'LIVE_destructure_write',
  'LIVE_find_write',
  'LIVE_recorded_callback',
  'LIVE_map_result',
  'LIVE_join_element',
  'LIVE_join_separator',
  'LIVE_at_argument',
  'LIVE_species',
  'LIVE_array_prototype_key',
]);
