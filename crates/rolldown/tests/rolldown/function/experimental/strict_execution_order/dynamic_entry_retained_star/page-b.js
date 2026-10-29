import assert from 'node:assert';
import { x } from './bridge.js';

x();
assert.strictEqual(
  globalThis.value,
  undefined,
  'dynamic retained-star re-export must not initialize common before page-b',
);

export function render() {}
