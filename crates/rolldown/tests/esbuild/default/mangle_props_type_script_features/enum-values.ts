enum TopLevelNumber { foo_ = 0 }
enum TopLevelString { bar_ = '' }
console.log({
	foo: TopLevelNumber.foo_,
	bar: TopLevelString.bar_,
})

function fn() {
	enum NestedNumber { foo_ = 0 }
	enum NestedString { bar_ = '' }
	console.log({
		foo: TopLevelNumber.foo_,
		bar: TopLevelString.bar_,
	})
}