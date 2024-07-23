import assert from 'node:assert'
class Foo {
	static #foo = 123 // This must be set before "bar" is initialized
	static bar = Foo.#foo
}
assert.equal(Foo.bar , 123)

class FooThis {
	static #foo = 123 // This must be set before "bar" is initialized
	static bar = this.#foo
}
assert.equal(FooThis.bar , 123)
