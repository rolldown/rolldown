class Foo {
	#foo
	#bar
	baz() {
		return [
			this.#foo,
			this.#bar,
			#foo in this,
		]
	}
}