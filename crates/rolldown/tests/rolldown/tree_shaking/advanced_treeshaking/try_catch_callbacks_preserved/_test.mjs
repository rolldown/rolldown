import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_arrow',
  'LIVE_function_expression',
  'LIVE_function_declaration',
  'LIVE_function_new',
  'LIVE_class_new',
  'LIVE_derived_new',
  'LIVE_method',
  'LIVE_nested_call',
  'LIVE_call_in_argument',
  'LIVE_inner_function',
  'LIVE_destructured',
  'LIVE_rest',
  'LIVE_optional_call',
  'LIVE_call_method',
  'LIVE_await',
  'LIVE_arguments',
  'LIVE_spread',
  'LIVE_other_module',
  'LIVE_finally_return',
  'LIVE_finally_continue',
]);
