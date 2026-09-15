import assert from 'node:assert';
import { common, _ } from './common.js';

assert.strictEqual(globalThis.value, 0);

export function render() {
  console.log(common, _);
}
