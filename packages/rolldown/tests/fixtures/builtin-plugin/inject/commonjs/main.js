import assert from 'node:assert';
import dep from './dep.cjs';
import tsDep from './dep.cts';

assert.strictEqual(dep.hasUseKey, false);
assert.strictEqual(tsDep.key, 'composable');
assert.strictEqual(useKey(), 'composable');
assert.strictEqual(Object.getPrototypeOf([]), Array.prototype);
