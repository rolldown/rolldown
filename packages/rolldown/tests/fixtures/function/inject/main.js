import assert from "node:assert";

assert.strictEqual(Promise, "promise-shim");
assert.strictEqual(P, "promise-shim");
assert.strictEqual($, "jquery");
assert.strictEqual(fs.default, "node-fs");
assert.strictEqual(Object.assign, "object-assign-shim");

// It should not inject shadowed variables.
(function (Promise, P, $, fs, Object) {
  assert.notEqual(Promise, "promise-shim");
  assert.notEqual(P, "promise-shim");
  assert.notEqual($, "jquery");
  assert.notEqual(fs.default, "node-fs");
  assert.notEqual(Object.assign, "object-assign-shim");
})();
