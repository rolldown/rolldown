import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_frozen_array_write',
  'LIVE_frozen_function_write',
  'LIVE_frozen_object_write',
  'LIVE_frozen_push',
  'LIVE_values_getter',
  'LIVE_entries_getter',
  'LIVE_values_null',
  'LIVE_entries_undefined',
  'LIVE_defined_getter',
  'LIVE_proxy_keys',
]);
