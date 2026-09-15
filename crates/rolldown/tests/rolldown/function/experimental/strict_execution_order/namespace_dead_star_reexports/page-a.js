import assert from 'node:assert';
import defaultValue from './default-only.js';
import { x } from './shadowed.js';

assert.strictEqual(globalThis.defaultValue, 0);
assert.strictEqual(globalThis.shadowedValue, 0);
assert.strictEqual(defaultValue, 42);
assert.strictEqual(x, 2);

export function render() {}
