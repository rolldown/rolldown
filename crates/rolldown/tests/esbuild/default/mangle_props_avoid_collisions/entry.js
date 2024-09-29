export default {
	foo_: 0, // Must not be named "a"
	bar_: 1, // Must not be named "b"
	a: 2,
	b: 3,
	__proto__: {}, // Always avoid mangling this
}