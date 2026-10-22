import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_proto_write',
  'LIVE_own_shadow',
  'LIVE_set_prototype',
  'LIVE_proto_getter',
  'LIVE_proto_getter',
  'LIVE_computed_proto_key',
  'LIVE_fn_prototype',
  'LIVE_object_prototype_key',
  'LIVE_object_prototype_chain',
  'LIVE_function_prototype_key',
]);
