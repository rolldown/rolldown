import assert from 'node:assert';
import { run } from './dist/main.js';

const { first, second, runs } = run();
assert.ok(first instanceof Error);
assert.strictEqual(first.message, 'member failed');
assert.strictEqual(second, first);
assert.strictEqual(runs, 1);
