import assert from 'node:assert';
import { data, load } from './dist/main.js';

assert.strictEqual(data.value, 42);
assert.strictEqual((await load()).default, data);
