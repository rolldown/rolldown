class Foo {
	static #foo = 123 // This must be set before "bar" is initialized
	static bar = Foo.#foo
}
console.log(Foo.bar === 123)

class FooThis {
	static #foo = 123 // This must be set before "bar" is initialized
	static bar = this.#foo
}
console.log(FooThis.bar === 123)