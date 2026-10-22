import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_tla_binding',
  'LIVE_tla_property',
  'LIVE_tla_function',
  'LIVE_eval_write',
  'LIVE_eval_read',
  'LIVE_eval_binding',
]);
