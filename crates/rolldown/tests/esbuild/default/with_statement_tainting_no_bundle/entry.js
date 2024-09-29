(() => {
	let local = 1
	let outer = 2
	let outerDead = 3
	with ({}) {
		var hoisted = 4
		let local = 5
		hoisted++
		local++
		if (1) outer++
		if (0) outerDead++
	}
	if (1) {
		hoisted++
		local++
		outer++
		outerDead++
	}
})()