(() => {
	function f() {}
	firstImportantSideEffect(f());
})();
(() => {
	function g() {}
	debugger;
	secondImportantSideEffect(g());
})();