import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_negative_zero',
  'LIVE_remainder_sign',
  'LIVE_nan',
  'LIVE_nan_power',
  'LIVE_one_power_infinity',
  'LIVE_negative_power',
  'LIVE_utf16_order',
  'LIVE_string_order',
  'LIVE_case_order',
  'LIVE_shift_count',
  'LIVE_unsigned_shift',
  'LIVE_int32_wrap',
  'LIVE_bit_not',
  'LIVE_string_concat',
  'LIVE_bool_add',
  'LIVE_typeof_regexp',
  'LIVE_value_of',
  'LIVE_subtract_object',
  'LIVE_to_string',
  'LIVE_compare_object',
  'LIVE_conditional_mixed',
  'LIVE_logical_fallback',
]);
