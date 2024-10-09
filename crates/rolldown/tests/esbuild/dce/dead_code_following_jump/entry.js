function testReturn() {
	if (true) return y + z()
	if (FAIL) return FAIL
	if (x) { var y }
	function z() { KEEP_ME() }
	return FAIL
}

function testThrow() {
	if (true) throw y + z()
	if (FAIL) return FAIL
	if (x) { var y }
	function z() { KEEP_ME() }
	return FAIL
}

function testBreak() {
	while (true) {
		if (true) {
			y + z()
			break
		}
		if (FAIL) return FAIL
		if (x) { var y }
		function z() { KEEP_ME() }
		return FAIL
	}
}

function testContinue() {
	while (true) {
		if (true) {
			y + z()
			continue
		}
		if (FAIL) return FAIL
		if (x) { var y }
		function z() { KEEP_ME() }
		return FAIL
	}
}

function testStmts() {
	return [a, b, c, d, e, f, g, h, i]

	while (x) { var a }
	while (FAIL) { let FAIL }

	do { var b } while (x)
	do { let FAIL } while (FAIL)

	for (var c; ;) ;
	for (let FAIL; ;) ;

	for (var d in x) ;
	for (let FAIL in FAIL) ;

	for (var e of x) ;
	for (let FAIL of FAIL) ;

	if (x) { var f }
	if (FAIL) { let FAIL }

	if (x) ; else { var g }
	if (FAIL) ; else { let FAIL }

	{ var h }
	{ let FAIL }

	x: { var i }
	x: { let FAIL }
}

testReturn()
testThrow()
testBreak()
testContinue()
testStmts()