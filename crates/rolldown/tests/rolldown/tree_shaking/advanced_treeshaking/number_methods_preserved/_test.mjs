import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_radix_low',
  'LIVE_radix_high',
  'LIVE_to_fixed_range',
  'LIVE_to_fixed_infinity',
  'LIVE_to_precision_range',
  'LIVE_to_exponential_range',
  'LIVE_radix_value_of',
  'LIVE_digits_value_of',
  'LIVE_extra_argument',
]);
