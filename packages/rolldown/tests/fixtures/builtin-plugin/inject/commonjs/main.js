import assert from 'node:assert';
import dep from './dep.cjs';

assert.strictEqual(dep.hasUseKey, false);
assert.strictEqual(useKey(), 'composable');
assert.strictEqual(Object.getPrototypeOf([]), Array.prototype);
