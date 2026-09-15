import assert from 'node:assert';
import * as bridge from './bridge.js';

bridge.x();
assert.strictEqual(
  globalThis.namespaceValue,
  undefined,
  'namespace member must not initialize its dead re-export before page-b',
);

export function render() {}
