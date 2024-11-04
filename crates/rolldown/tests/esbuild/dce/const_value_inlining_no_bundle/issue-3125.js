function foo() {
	const f = () => x
	const x = 0
	return f()
}

assert(foo() !== undefined)
