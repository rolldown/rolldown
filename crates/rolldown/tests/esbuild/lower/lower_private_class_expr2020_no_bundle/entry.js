export let Foo = class {
	#field
	#method() {}
	static #staticField
	static #staticMethod() {}
	foo() {
		this.#field = this.#method()
		Foo.#staticField = Foo.#staticMethod()
	}
}