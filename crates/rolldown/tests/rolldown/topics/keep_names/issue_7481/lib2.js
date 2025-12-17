import assert from "node:assert";
var _Test;
let Test = (_Test = class {});

var _Fn;
let Fn = (_Fn = function () {
  return 1;
});

var _ArrowFn;
let ArrowFn = (_ArrowFn = () => {
  return 2;
});

console.log(_Test, Test, _Fn, Fn, _ArrowFn, ArrowFn);
assert.strictEqual(_Test.name, "_Test");
assert.strictEqual(_Fn.name, "_Fn");
assert.strictEqual(_ArrowFn.name, "_ArrowFn");

assert.strictEqual(new _Test() instanceof Test, true);
assert.strictEqual(Fn(), 1);
assert.strictEqual(ArrowFn(), 2);
assert.strictEqual(_Fn(), 1);
assert.strictEqual(_ArrowFn(), 2);
