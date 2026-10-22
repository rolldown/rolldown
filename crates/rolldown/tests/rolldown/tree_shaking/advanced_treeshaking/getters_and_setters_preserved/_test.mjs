import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_getter_this',
  'LIVE_getter_binding_write',
  'LIVE_getter_state',
  'LIVE_getter_throw',
  'LIVE_setter_throw',
  'LIVE_setter_write',
  'LIVE_setter_this',
  'LIVE_getter_only_write',
  'LIVE_overwrite_value',
  'LIVE_overwrite_getter',
]);
