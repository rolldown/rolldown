async function foo(bar) {
	await bar
	return [this, arguments]
}
class Foo {async foo() {}}
export default [
	foo,
	Foo,
	async function() {},
	async () => {},
	{async foo() {}},
	class {async foo() {}},
	function() {
		return async (bar) => {
			await bar
			return [this, arguments]
		}
	},
]