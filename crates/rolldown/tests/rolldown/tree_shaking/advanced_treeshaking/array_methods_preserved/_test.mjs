import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_sort_to_string',
  'LIVE_sort_comparator',
  'LIVE_sort_result',
  'LIVE_reduce_empty',
  'LIVE_reduce_right_holes',
  'LIVE_reduce_element',
  'LIVE_reduce_start',
  'LIVE_reduce_accumulator',
  'LIVE_splice_start',
  'LIVE_copy_within_target',
  'LIVE_fill_start',
  'LIVE_concat_spreadable',
  'LIVE_flat_proxy',
  'LIVE_flat_map_proxy',
  'LIVE_own_sort',
  'LIVE_concat_species',
  'LIVE_reverse_read',
  'LIVE_splice_read',
  'LIVE_splice_insert',
  'LIVE_copy_within_read',
  'LIVE_fill_read',
  'LIVE_sort_read',
]);
