import assert from 'node:assert/strict';
import { arrow } from './arrow.js';
import { hoisted } from './hoisted.js';
import { getFoo } from './top-level.js';

assert.strictEqual(await arrow(), 'foo');
assert.strictEqual(await hoisted(), 'foo');
assert.strictEqual(getFoo(), 'foo');
