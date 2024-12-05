import assert from 'node:assert'

var Foo = class {}
assert.strictEqual(Foo.name, "Foo")

var fn = function() {}
assert.strictEqual(fn.name, "fn")