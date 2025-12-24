import assert from "node:assert";
class _Test {}
let _Fn = function () {};
let _ArrowFn = () => {};
assert.strictEqual(_Test.name, "_Test");
assert.strictEqual(_Fn.name, "_Fn");
assert.strictEqual(_ArrowFn.name, "_ArrowFn");
