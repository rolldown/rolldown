class Bar {
	a
	declare b
	[(() => null, c)]
	declare [(() => null, d)]

	static A
	static declare B
	static [(() => null, C)]
	static declare [(() => null, D)]
}
(() => new Bar())()