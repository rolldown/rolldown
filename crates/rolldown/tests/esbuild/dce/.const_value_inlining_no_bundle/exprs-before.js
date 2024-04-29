function nested() {
	const x = [, '', {}, 0n, /./, function() {}, () => {}]
	const y_REMOVE = 1
	function foo() {
		return y_REMOVE
	}
}