console.log(
	// These should be fully substituted
	import.meta,
	import.meta.foo,
	import.meta.foo.bar,

	// Should just substitute "import.meta.foo"
	import.meta.foo.baz,

	// This should not be substituted
	import.meta.bar,
)