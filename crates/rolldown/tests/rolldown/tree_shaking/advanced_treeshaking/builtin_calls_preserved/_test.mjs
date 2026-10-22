import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_string_argument',
  'LIVE_concat_argument',
  'LIVE_symbol_replace',
  'LIVE_replace_callback',
  'LIVE_regexp_argument',
  'LIVE_radix_throw',
  'LIVE_to_fixed_throw',
  'LIVE_repeat_throw',
  'LIVE_symbol_throw',
  'LIVE_own_method',
  'LIVE_proto_method',
  'LIVE_frozen_write',
  'LIVE_frozen_add',
  'LIVE_frozen_delete',
  'LIVE_freeze_shallow',
  'LIVE_frozen_push',
  'LIVE_frozen_read',
  'LIVE_regexp_state',
]);
