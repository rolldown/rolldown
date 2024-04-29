class Foo {
	#x
	foo() {
		this?.#x.y
		this?.y.#x
		this.#x?.y
	}
}