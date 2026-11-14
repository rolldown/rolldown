import assert from 'node:assert';
import { collision as a } from './common-a.js';
import { collision as b } from './common-b.js';

assert.strictEqual(globalThis.valueA, 0);
assert.strictEqual(globalThis.valueB, 0);
assert.strictEqual(a, 'a');
assert.strictEqual(b, 'b');

export function render() {}
