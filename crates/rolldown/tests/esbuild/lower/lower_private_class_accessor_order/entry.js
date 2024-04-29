class Foo {
	bar = this.#foo
	get #foo() { return 123 } // This must be set before "bar" is initialized
}
console.log(new Foo().bar === 123)