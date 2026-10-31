import assert from 'node:assert';
import { data } from './main.js';

// This read happens before main.js runs its body.
assert.strictEqual(data.value, 42);
