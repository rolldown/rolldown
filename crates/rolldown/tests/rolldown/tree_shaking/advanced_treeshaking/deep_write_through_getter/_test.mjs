import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_object_getter',
  'LIVE_proto_getter',
  'LIVE_static_getter',
  'LIVE_inherited_static_getter',
  'LIVE_instance_getter',
  'LIVE_nested_getter',
  'LIVE_getter_literal',
  'LIVE_function_return',
  'LIVE_method_return',
  'LIVE_arrow_return',
]);
