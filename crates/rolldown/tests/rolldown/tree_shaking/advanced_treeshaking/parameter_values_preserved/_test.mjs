import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.resolve(import.meta.dirname, 'dist');
const output = fs
  .readdirSync(dist)
  .filter((file) => file.endsWith('.js'))
  .map((file) => fs.readFileSync(path.join(dist, file), 'utf-8'))
  .join('\n');

import { done } from './dist/main.js';
assert.strictEqual(done, true);
assert.deepStrictEqual(globalThis.log, [
  'LIVE_mixed',
  'LIVE_viaB',
  'LIVE_reassigned',
  'LIVE_param_write',
  'LIVE_alias_write',
  'LIVE_return_write',
  'LIVE_later_call',
  'LIVE_callback',
  'LIVE_call',
  'LIVE_default',
  'LIVE_spread',
  'LIVE_arguments',
  'LIVE_recursion',
  'LIVE_holderless',
  'LIVE_getter',
  'LIVE_destructure_getter',
  'LIVE_local_fn',
  'LIVE_closure_param',
]);
assert.deepStrictEqual(globalThis.made, { a: 'LIVE_return_escape', b: 1 });
// Code that this run does not reach, but other callers can reach.
assert.ok(output.includes('LIVE_viaA'));
assert.ok(output.includes('LIVE_escaped'));
