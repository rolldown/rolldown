import assert from 'node:assert';
import * as bridge from './bridge.js';

assert.strictEqual(bridge.x, 1);
assert.strictEqual(
  globalThis.inlineNamespaceValue,
  undefined,
  'inlined namespace member must not initialize its dead re-export before page-b',
);

export function render() {}
