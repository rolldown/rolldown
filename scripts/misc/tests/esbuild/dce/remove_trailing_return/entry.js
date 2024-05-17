function foo() {
	if (a) b()
	return
}
function bar() {
	if (a) b()
	return KEEP_ME
}
export default [
	foo,
	bar,
	function () {
		if (a) b()
		return
	},
	function () {
		if (a) b()
		return KEEP_ME
	},
	() => {
		if (a) b()
		return
	},
	() => {
		if (a) b()
		return KEEP_ME
	},
]