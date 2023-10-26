ok(
	// These should be fully substituted
	this,
	this.foo,
	this.foo.bar,

	// Should just substitute "this.foo"
	this.foo.baz,

	// This should not be substituted
	this.bar,
);

// This code should be the same as above
(() => {
	ok(
		this,
		this.foo,
		this.foo.bar,
		this.foo.baz,
		this.bar,
	);
})();

// Nothing should be substituted in this code
(function() {
	doNotSubstitute(
		this,
		this.foo,
		this.foo.bar,
		this.foo.baz,
		this.bar,
	);
})();