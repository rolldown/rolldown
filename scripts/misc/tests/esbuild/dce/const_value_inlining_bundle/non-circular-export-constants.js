const foo = 123 // Inlining should be prevented by the cycle
function bar() {
	return foo
}
export { foo, bar }