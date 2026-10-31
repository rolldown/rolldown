import assert from 'node:assert';
import { reads } from './flag.js';
import { calls, run } from './run.js';
import { derived } from './derived.js';

assert.deepStrictEqual(reads, [false, true]);

run();
assert.deepStrictEqual(calls, ['second']);

assert.strictEqual(derived, 2);
