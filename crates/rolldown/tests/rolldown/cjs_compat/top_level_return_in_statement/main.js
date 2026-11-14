import assert from 'node:assert';
import './return_in_if.cjs';

assert.strictEqual(globalThis.afterReturnInIf, undefined);
