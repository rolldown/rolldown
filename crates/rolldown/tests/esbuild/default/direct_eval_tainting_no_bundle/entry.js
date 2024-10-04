function test1() {
	function add(first, second) {
		return first + second
	}
	eval('add(1, 2)')
}

function test2() {
	function add(first, second) {
		return first + second
	}
	(0, eval)('add(1, 2)')
}

function test3() {
	function add(first, second) {
		return first + second
	}
}

function test4(eval) {
	function add(first, second) {
		return first + second
	}
	eval('add(1, 2)')
}

function test5() {
	function containsDirectEval() { eval() }
	if (true) { var shouldNotBeRenamed }
}