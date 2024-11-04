function foo() {
	const x = y
	const y = 1
	console.log(
		x, x,
		y, y,
	)
}

assert(foo() !== undefined) // ensure this is not removed by DCE
