import assert from 'node:assert';
import './in_if.cjs';
import './return_in_if.cjs';

assert.strictEqual(globalThis.effectInIf, true);
assert.strictEqual(globalThis.afterReturn, undefined);
