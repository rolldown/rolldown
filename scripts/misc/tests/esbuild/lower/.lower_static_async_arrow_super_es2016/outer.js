// Helper functions for "super" shouldn't be inserted into this outer function
export default (async function () {
	class y extends z {
		static foo = async () => super.foo()
	}
	await y.foo()()
})()