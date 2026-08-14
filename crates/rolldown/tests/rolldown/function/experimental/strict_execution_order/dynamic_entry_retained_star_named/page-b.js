import assert from 'node:assert';
import { x } from './bridge.js';

x();
assert.strictEqual(globalThis.value, undefined);

export function render() {}
