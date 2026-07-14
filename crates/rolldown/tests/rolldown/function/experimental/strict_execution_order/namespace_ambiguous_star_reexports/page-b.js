import assert from 'node:assert';
import * as bridge from './bridge.js';

assert.strictEqual(bridge[globalThis.__bridgeKey || 'ok'](), 1);
assert.strictEqual(
  globalThis.valueA,
  undefined,
  'ambiguous namespace re-export must not initialize common-a before page-b',
);
assert.strictEqual(globalThis.valueB, undefined);

export function render() {}
