function nested() {
	'directive'
	const REMOVE = 1
	x = [REMOVE, REMOVE]
}


assert(nested() !== undefined) // ensure this is not removed by DCE
