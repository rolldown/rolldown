import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_after_break',
  'LIVE_check_effect',
  'LIVE_labeled',
  'LIVE_switch_break',
  'LIVE_break_finally',
  'LIVE_return_in_loop',
  'LIVE_do_once',
  'LIVE_for_init',
  'LIVE_for_in_a',
  'LIVE_for_in_inherited',
  'LIVE_optional_short_circuit',
  'LIVE_optional_call',
  'LIVE_optional_effect',
  'LIVE_optional_after_write',
]);
