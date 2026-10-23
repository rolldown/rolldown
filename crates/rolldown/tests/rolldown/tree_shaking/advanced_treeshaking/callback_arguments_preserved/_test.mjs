import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_unary_value_of',
  'LIVE_bitwise_value_of',
  'LIVE_template_to_string',
  'LIVE_sort_value_of',
  'LIVE_written_element',
  'LIVE_pushed_element',
  'LIVE_array_parameter_write',
  'LIVE_direct_call',
  'LIVE_this_argument',
  'LIVE_element_getter',
  'LIVE_index_value',
  'LIVE_reduce_total',
]);
