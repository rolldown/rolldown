import assert from 'node:assert';
import { x } from './bridge.js';

x();
assert.strictEqual(
  globalThis.valueA,
  undefined,
  'dynamic retained-star fanout must not initialize common-a before page-b',
);
assert.strictEqual(globalThis.valueB, undefined);

export function render() {}
