import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_new_method',
  'LIVE_new_arrow',
  'LIVE_new_async',
  'LIVE_new_generator',
  'LIVE_new_class_method',
  'LIVE_new_static_method',
  'LIVE_parent_constructor',
  'LIVE_derived_field',
  'LIVE_es5_parent',
  'LIVE_parent_constructor',
  'LIVE_extends_null',
  'LIVE_frozen_parent',
  'LIVE_frozen_es5_parent',
  'LIVE_frozen_grandparent',
  'LIVE_private_field_twice',
  'LIVE_private_method_twice',
  'LIVE_function_expression',
  'LIVE_holder_constructor',
]);
