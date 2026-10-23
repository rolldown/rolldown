import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_tdz_closure',
  'LIVE_tdz_hoisted',
  'LIVE_tdz_callback',
  'LIVE_tdz_typeof',
  'LIVE_case_clause',
  'LIVE_let_write',
  'LIVE_closure_value',
]);
