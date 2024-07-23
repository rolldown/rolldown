import assert from 'node:assert'
class Foo {
	#foo = 123 // This must be set before "bar" is initialized
	bar = this.#foo
}
assert.equal(new Foo().bar, 123)
