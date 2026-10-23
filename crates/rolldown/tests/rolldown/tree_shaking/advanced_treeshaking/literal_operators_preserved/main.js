import {
  astral,
  lastBmp,
  lower,
  nan,
  named,
  negativeZero,
  nine,
  object,
  one,
  ten,
  upper,
} from './values.js';

globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// The folding gives the exact results.
if (one / negativeZero === -Infinity) log('LIVE_negative_zero');
if (-one % one === 0 && one / (-one % one) === -Infinity) log('LIVE_remainder_sign');
if (nan !== nan) log('LIVE_nan');
if (nan ** 0 === 1) log('LIVE_nan_power');
if (one ** Infinity !== one ** Infinity) log('LIVE_one_power_infinity');
if ((-one) ** 3 === -1) log('LIVE_negative_power');
if (astral < lastBmp) log('LIVE_utf16_order');
if (ten < nine) log('LIVE_string_order');
if (upper < lower) log('LIVE_case_order');
if (one << 32 === 1) log('LIVE_shift_count');
if (-one >>> 0 === 4294967295) log('LIVE_unsigned_shift');
if ((one * 2147483648) | 0) log('LIVE_int32_wrap');
if (~one === -2) log('LIVE_bit_not');
if (one + '1' === '11') log('LIVE_string_concat');
if (one + true === 2) log('LIVE_bool_add');
if (typeof /x/ === 'object') log('LIVE_typeof_regexp');

// A conversion of an object runs code.
if (object - 1 === 0) log('LIVE_subtract_object');
if (named < 'z') log('LIVE_compare_object');

// An unknown condition with different alternatives.
const mixed = globalThis.missing ? false : true;
if (mixed) log('LIVE_conditional_mixed');
const fallback = globalThis.missing || 'fallback';
if (fallback === 'fallback') log('LIVE_logical_fallback');

export const done = true;
