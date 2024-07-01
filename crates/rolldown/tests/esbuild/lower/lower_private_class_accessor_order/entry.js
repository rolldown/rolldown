import assert from 'node:assert'
class Foo {
	bar = this.#foo
	get #foo() { return 123 } // This must be set before "bar" is initialized
}
assert(new Foo().bar === 123)
