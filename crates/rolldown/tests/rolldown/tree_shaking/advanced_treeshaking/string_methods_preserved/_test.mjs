import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_repeat_negative',
  'LIVE_repeat_infinity',
  'LIVE_repeat_length',
  'LIVE_pad_start_length',
  'LIVE_pad_end_length',
  'LIVE_normalize_form',
  'LIVE_match_pattern',
  'LIVE_search_pattern',
  'LIVE_match_all_flags',
  'LIVE_match_all_pattern',
  'LIVE_locale_compare_tag',
  'LIVE_locale_lower_tag',
  'LIVE_locale_upper_tag',
  'LIVE_repeat_count',
  'LIVE_pad_fill',
  'LIVE_normalize_to_string',
  'LIVE_custom_matcher',
  'LIVE_custom_searcher',
  'LIVE_match_last_index',
  'LIVE_match_reset_last_index',
]);
