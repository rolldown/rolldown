class Foo {
	#x
	unary() {
		this.#x++
		this.#x--
		++this.#x
		--this.#x
	}
	binary() {
		this.#x = 1
		this.#x += 1
		this.#x -= 1
		this.#x *= 1
		this.#x /= 1
		this.#x %= 1
		this.#x **= 1
		this.#x <<= 1
		this.#x >>= 1
		this.#x >>>= 1
		this.#x &= 1
		this.#x |= 1
		this.#x ^= 1
		this.#x &&= 1
		this.#x ||= 1
		this.#x ??= 1
	}
}