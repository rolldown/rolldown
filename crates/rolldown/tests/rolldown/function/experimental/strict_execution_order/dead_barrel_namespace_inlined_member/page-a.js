import assert from 'node:assert';
import { x, y } from './constants.js';

assert.strictEqual(globalThis.inlineNamespaceValue, 0);

export function render() {
  console.log(x, y);
}
