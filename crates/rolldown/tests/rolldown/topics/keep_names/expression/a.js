import assert from 'node:assert'

var Foo = class {}, Bar = class {}
assert.strictEqual(Foo.name, "Foo")
assert.strictEqual(Bar.name, "Bar")

var fn = function() {}, fn2 = function() {}
assert.strictEqual(fn.name, "fn")
assert.strictEqual(fn2.name, "fn2")

