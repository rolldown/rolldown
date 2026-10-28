import assert from 'node:assert';
import a from './a.cjs';
import './after.js';

assert.deepStrictEqual(globalThis.__events, ['a', 'b', 'after']);
assert.strictEqual(a.seen, 'b');
