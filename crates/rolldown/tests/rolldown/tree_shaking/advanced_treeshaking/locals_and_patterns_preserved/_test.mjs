import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_closure_write',
  'LIVE_closure_counter',
  'LIVE_local_set',
  'LIVE_destructure_getter',
  'LIVE_rest_getter',
  'LIVE_rest_param_getter',
  'LIVE_nested_rest_getter',
  'LIVE_assignment_rest_getter',
  'LIVE_rest_copy',
  'LIVE_pattern_after_write',
  'LIVE_default_value',
]);
