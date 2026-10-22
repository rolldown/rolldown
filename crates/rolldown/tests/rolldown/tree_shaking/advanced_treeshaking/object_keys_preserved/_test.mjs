import assert from 'node:assert';

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_symbol_getter',
  'LIVE_symbol_write',
  'LIVE_symbol_identity',
  'LIVE_delete_read',
  'LIVE_key_order_ab',
]);
