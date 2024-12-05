import assert from 'node:assert'

var Foo = class Foo {} 
assert.strictEqual(Foo.name, "Foo")

var fn = function fn() {}
assert.strictEqual(fn.name, "fn")