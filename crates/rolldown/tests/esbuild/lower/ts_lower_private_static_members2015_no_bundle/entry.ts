class Foo {
	static #x
	static get #y() {}
	static set #y(x) {}
	static #z() {}
	foo() {
		Foo.#x += 1
		Foo.#y += 1
		Foo.#z()
	}
}