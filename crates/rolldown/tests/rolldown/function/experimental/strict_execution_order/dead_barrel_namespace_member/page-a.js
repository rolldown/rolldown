import assert from 'node:assert';
import { common, _ } from './common.js';

assert.strictEqual(globalThis.namespaceValue, 0);

export function render() {
  console.log(common, _);
}
