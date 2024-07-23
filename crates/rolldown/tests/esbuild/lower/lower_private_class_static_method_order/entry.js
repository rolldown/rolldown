import assert from 'node:assert'
class Foo {
	static bar = Foo.#foo()
	static #foo() { return 123 } // This must be set before "bar" is initialized
}
assert(Foo.bar === 123)

class FooThis {
	static bar = this.#foo()
	static #foo() { return 123 } // This must be set before "bar" is initialized
}
assert(FooThis.bar === 123)
