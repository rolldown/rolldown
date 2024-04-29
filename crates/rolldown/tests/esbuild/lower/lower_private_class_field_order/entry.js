class Foo {
	#foo = 123 // This must be set before "bar" is initialized
	bar = this.#foo
}
console.log(new Foo().bar === 123)