import * as cjsNamespace from './cjs-module.js';
import assert from 'node:assert';

// Static import ensures the CJS module is in the same chunk
assert.strictEqual(cjsNamespace.value, 42);

// Dynamic import of a CJS module that's already in the same chunk
// This should be rewritten to: Promise.resolve().then(() => __toESM(require_cjs_module()))
import('./cjs-module.js').then((mod) => {
  assert.strictEqual(mod.value, 42);
  assert.strictEqual(mod.default.value, 42);
});
