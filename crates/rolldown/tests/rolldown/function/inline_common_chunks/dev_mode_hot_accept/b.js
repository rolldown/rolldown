import assert from 'node:assert';
import { value } from './self_accept.js';
import { current } from './accept_dep.js';

assert.strictEqual(value, 'self-v0');
assert.strictEqual(current(), 'dep-v0');
