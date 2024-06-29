import assert from 'node:assert'
class Foo {
	bar = this.#foo()
	#foo() { return 123 } // This must be set before "bar" is initialized
}
assert.equal(new Foo().bar, 123)
