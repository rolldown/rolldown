import assert from 'node:assert';
import { x } from './bridge.js';

x();
assert.strictEqual(globalThis.valueA, undefined);
assert.strictEqual(globalThis.valueB, undefined);

export function render() {}
