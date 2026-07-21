import assert from 'node:assert';
import * as bridge from './bridge.js';

assert.strictEqual(bridge[globalThis.__bridgeKey || 'x'](), 1);
assert.strictEqual(
  globalThis.defaultValue,
  undefined,
  'dead namespace star must not initialize default-only before page-b',
);
assert.strictEqual(globalThis.shadowedValue, undefined);

export function render() {}
