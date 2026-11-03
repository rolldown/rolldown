import assert from 'node:assert';
import { x, common as unused } from './bridge.js';

x();
assert.strictEqual(
  globalThis.value,
  undefined,
  'dead barrel re-export must not initialize common before page-b',
);

export function render() {}
