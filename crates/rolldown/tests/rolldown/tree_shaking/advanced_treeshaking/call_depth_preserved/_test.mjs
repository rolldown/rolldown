import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_effect_chain',
  'LIVE_true_chain',
  'LIVE_chain_getter',
  'LIVE_chain_write',
  'LIVE_recursion',
  'LIVE_mutual_recursion',
]);
