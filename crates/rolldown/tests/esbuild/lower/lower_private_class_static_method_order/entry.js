class Foo {
	static bar = Foo.#foo()
	static #foo() { return 123 } // This must be set before "bar" is initialized
}
console.log(Foo.bar === 123)

class FooThis {
	static bar = this.#foo()
	static #foo() { return 123 } // This must be set before "bar" is initialized
}
console.log(FooThis.bar === 123)